use log::{debug, info, warn};
use std::{env, fs, path::PathBuf, process};

mod error;
mod parser;
mod runner;
mod task_model;

use crate::{
    error::RunnerError,
    parser::parse_tasks_yaml,
    runner::{RunOptions, Section, run_tasks},
};
use colored::Colorize;

use clap::{Parser, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "zmake-tasks-runner", version, about)]
struct Cli {
    /// Path to YAML file. Defaults to ZMake.yml if not provided.
    #[arg(value_name = "FILE", default_value = "ZMake.yml")]
    file: PathBuf,

    /// Working directory to run commands in. Defaults to current directory.
    #[arg(long = "cwd", value_name = "DIR")]
    cwd: Option<PathBuf>,

    /// Override detected OS (advanced). By default detected from std::env::consts::OS.
    #[arg(long = "os", value_enum)]
    os: Option<OsChoice>,
    #[arg(long = "section", value_enum)]
    sections: Vec<Section>,

    /// Continue executing remaining commands when one fails. By default, fails fast.
    #[arg(long = "continue-on-error")]
    continue_on_error: bool,

    /// Print the commands without executing them.
    #[arg(long = "dry-run")]
    dry_run: bool,

    /// Extra environment variables for child processes (KEY=VALUE). Can be repeated.
    #[arg(long = "env", value_name = "KV", value_parser = parse_kv)]
    envs: Vec<(String, String)>,

    /// Extra environment variables for child processes from a file (KEY=VALUE per line).
    #[arg(long = "env-file", value_name = "FILE")]
    env_file: Option<PathBuf>,

    /// Increase verbosity. Repeat for more detail (-v, -vv, -vvv).
    #[arg(short = 'v', long = "verbose", action = clap::ArgAction::Count)]
    verbose: u8,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OsChoice {
    Windows,
    Linux,
    Macos,
}

fn parse_kv(s: &str) -> Result<(String, String), String> {
    let (k, v) = s
        .split_once('=')
        .ok_or_else(|| "expected KEY=VALUE".to_string())?;
    if k.is_empty() {
        return Err("key cannot be empty".into());
    }
    Ok((k.to_string(), v.to_string()))
}

fn main() {
    if let Err(e) = real_main() {
        eprintln!("Error: {e}");
        process::exit(1);
    }
}

fn real_main() -> Result<(), RunnerError> {
    let mut cli = Cli::parse();

    let level = match cli.verbose {
        0 => "info",
        1 => "debug",
        _ => "trace",
    };

    unsafe {
        std::env::set_var("RUST_LOG", format!("zmake_tasks_runner={level},info"));
        let _ = env_logger::try_init();
    }

    let yaml = fs::read_to_string(&cli.file)?;
    debug!("loaded yaml from {:?}", cli.file);
    let tasks = parse_tasks_yaml(&yaml)?;

    let detected_os = env::consts::OS;

    if detected_os != "windows" && detected_os != "linux" && detected_os != "macos" {
        return Err(RunnerError::CmdFailed(format!(
            "unsupported OS detected: {}",
            detected_os
        )));
    }

    let os = match cli.os {
        Some(OsChoice::Windows) => "windows",
        Some(OsChoice::Linux) => "linux",
        Some(OsChoice::Macos) => "macos",
        None => detected_os,
    };

    if detected_os != os {
        warn!(
            "{}",
            format!(
                "Overriding detected OS '{}' with user-specified OS '{}'. We are forcing dry-run mode.",
                detected_os, os
            )
            .yellow()
        );
        cli.dry_run = true;
    }

    let cwd = cli
        .cwd
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let mut extra_env = std::collections::HashMap::new();
    for (k, v) in cli.envs {
        extra_env.insert(k, v);
    }
    if let Some(env_file) = cli.env_file {
        let content = fs::read_to_string(env_file)?;
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (k, v) = parse_kv(line)
                .map_err(|e| RunnerError::CmdFailed(format!("env-file parse error: {}", e)))?;
            extra_env.insert(k, v);
        }
    }

    let opts = RunOptions {
        os,
        cwd: Some(cwd),
        extra_env,
        continue_on_error: cli.continue_on_error,
        dry_run: cli.dry_run,
        sections: if cli.sections.is_empty() {
            None
        } else {
            Some(cli.sections)
        },
    };

    run_tasks(&tasks, &opts)?;
    info!(
        "{}",
        format_args!("{}", "All tasks completed successfully.".green())
    );
    Ok(())
}
