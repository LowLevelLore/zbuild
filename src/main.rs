use colored::Colorize;
use log::{error, info, warn};
use std::{
    env, fs,
    path::PathBuf,
    process::{self},
};
mod config_model;
mod environment;
mod error;
mod parser;
mod runner;

use crate::{
    environment::{EnvVariableSource, Environment},
    error::RunnerError,
    runner::{Section, run},
};
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
        error!("{}", format!("Error: {}", e).red());
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

    let mut default_environment = Environment::default();

    let _ = default_environment.capture_default_environment();

    let mut global_environment = default_environment.clone();

    if let Some(global_config) = &config.global_config {
        if let Some(exec_policy) = &global_config.execution_policy {
            global_environment.execution_policy = exec_policy.clone();
        }
        if let Some(env_vars) = &global_config.env {
            for (key, value) in env_vars {
                global_environment.upsert_variable(
                    key.clone(),
                    value.clone(),
                    EnvVariableSource::Global,
                );
            }
        }
    }

    for (k, v) in cli.envs {
        global_environment.upsert_variable(k, v, environment::EnvVariableSource::Passed);
    }
    if let Some(env_file) = cli.env_file {
        let content = fs::read_to_string(env_file)?;
        global_environment.load_env(content, environment::EnvVariableSource::Passed);
    }

    global_environment.os = os;
    global_environment.cwd = Some(cwd);
    global_environment.sections = if cli.sections.is_empty() {
        None
    } else {
        Some(cli.sections)
    };
    if let Some(global_config) = &config.global_config
        && let Some(banned_sections) = &global_config.banned_sections
    {
        global_environment.banned_sections = Some(
            banned_sections
                .iter()
                .map(|section| Section::get_section(Section::map_section(section)))
                .collect(),
        );
    }

    match run(&config, &mut global_environment) {
        Ok(_) => {
            info!(
                "{}",
                format_args!("{}", "All tasks completed successfully.".green())
            );
        }
        Err(e) => {
            return Err(e);
        }
    }

    Ok(())
}
