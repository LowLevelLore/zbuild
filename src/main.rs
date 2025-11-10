use log::{debug, info, warn};
use std::{
    env, fs,
    path::PathBuf,
    process::{self, Command, Stdio},
};

mod config_model;
mod error;
mod parser;
mod runner;

use crate::{
    error::RunnerError,
    runner::{RunOptions, Section, run},
};
use clap::{Parser, ValueEnum};
use colored::Colorize;

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
    #[arg(long = "env", value_name = "KV", value_parser = parser::parse_kv)]
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
    let config = parser::parse_yaml(&yaml)?;

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
            let (k, v) = parser::parse_kv(line)
                .map_err(|e| RunnerError::CmdFailed(format!("env-file parse error: {}", e)))?;
            extra_env.insert(k, v);
        }
    }

    let mut opts = RunOptions {
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

    let mut cmd = if opts.os == "windows" {
        let mut c = Command::new("cmd");
        c.arg("/C").arg("set > .env.vars.zbuild");
        c.env("TERM", "xterm-256color");
        c.env("ANSICON", "1");
        c
    } else {
        let mut c = Command::new("sh");
        c.arg("-c").arg("env > .env.vars.zbuild");
        c.env("TERM", "xterm-256color");
        c
    };

    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;

    let status = child.wait()?;

    if !status.success() {
        return Err(RunnerError::CmdFailed(
            "failed to initialize environment variables".to_string(),
        ));
    }

    let env_vars_path = if let Some(ref dir) = opts.cwd {
        dir.join(".env.vars.zbuild")
    } else {
        PathBuf::from(".env.vars.zbuild")
    };

    if env_vars_path.exists()
        && let Ok(content) = std::fs::read_to_string(&env_vars_path)
    {
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                opts.extra_env.insert(k.to_string(), v.to_string());
            }
        }
    }

    if env_vars_path.exists() {
        let _ = std::fs::remove_file(&env_vars_path);
    }

    run(&config, &mut opts)?;
    info!(
        "{}",
        format_args!("{}", "All tasks completed successfully.".green())
    );
    Ok(())
}
