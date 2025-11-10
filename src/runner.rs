use log::{info, warn};
use std::{
    collections::HashMap,
    path::PathBuf,
    process::{Command, ExitStatus, Stdio},
};

use colored::Colorize;

use crate::{
    config_model::{Config, PlatformCommands},
    error::RunnerError,
};
use clap::ValueEnum;

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq, Hash)]
pub enum Section {
    PreBuild,
    Build,
    PostBuild,
    Test,
    PreDeploy,
    Deploy,
    PostDeploy,
    Clean,
}

impl Section {
    pub fn as_str(&self) -> &'static str {
        match self {
            Section::PreBuild => "PreBuild",
            Section::Build => "Build",
            Section::PostBuild => "PostBuild",
            Section::Test => "Test",
            Section::PreDeploy => "PreDeploy",
            Section::Deploy => "Deploy",
            Section::PostDeploy => "PostDeploy",
            Section::Clean => "Clean",
        }
    }

    fn get_section(name: &str) -> Section {
        match name {
            "PreBuild" => Section::PreBuild,
            "Build" => Section::Build,
            "PostBuild" => Section::PostBuild,
            "Test" => Section::Test,
            "PreDeploy" => Section::PreDeploy,
            "Deploy" => Section::Deploy,
            "PostDeploy" => Section::PostDeploy,
            "Clean" => Section::Clean,
            _ => panic!("unknown section name: {}", name),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RunOptions<'a> {
    /// OS to target: "windows" | "linux" | "macos"
    pub os: &'a str,
    /// Working directory for all commands.
    pub cwd: Option<PathBuf>,
    pub extra_env: HashMap<String, String>,
    /// If true, keep going after failures and report at end.
    pub continue_on_error: bool,
    /// If true, print commands but don't execute.
    pub dry_run: bool,
    /// Optional subset of sections to execute (preserving original order).
    pub sections: Option<Vec<Section>>,
}

fn commands_for_os<'a>(pc: &'a PlatformCommands, os: &str) -> Option<&'a Vec<String>> {
    match os {
        "windows" => pc.windows.steps.as_ref(),
        "linux" => pc.linux.steps.as_ref(),
        "macos" => pc.macos.steps.as_ref(),
        _ => None,
    }
}

pub fn run(config: &Config, opts: &mut RunOptions) -> Result<(), RunnerError> {
    let mut overall_failures: Vec<String> = Vec::new();
    let filter: Option<std::collections::HashSet<&'static str>> = opts
        .sections
        .as_ref()
        .map(|v| v.iter().map(|s| s.as_str()).collect());
    for (section_name, commands) in config.tasks.ordered_sections() {
        if let Some(ref filt) = filter {
            if !filt.contains(&section_name) {
                continue;
            }
        } else if section_name == Section::Clean.as_str()
            || section_name == Section::PostDeploy.as_str()
            || section_name == Section::Deploy.as_str()
        {
            continue;
        }
        match commands {
            Some(c) => {
                let result = match commands_for_os(c, opts.os) {
                    Some(cmds) => run_section(section_name, config, cmds, opts),
                    None => {
                        continue;
                    }
                };
                match result {
                    Ok(_) => {}
                    Err(e) => {
                        if opts.continue_on_error {
                            warn!(
                                "{}",
                                format!("section '{}' failed with error: {}", section_name, e)
                                    .yellow()
                            );
                            overall_failures.push(format!("section '{}': {:?}", section_name, e));
                        } else {
                            return Err(e);
                        }
                    }
                }
            }
            None => {
                continue;
            }
        };
    }
    if !overall_failures.is_empty() {
        return Err(RunnerError::CmdFailed(overall_failures.join("\n")));
    }
    Ok(())
}

pub fn run_section(
    section_name: &str,
    config: &Config,
    tasks: &Vec<String>,
    opts: &mut RunOptions,
) -> Result<(), RunnerError> {
    info!(
        "{}",
        format!(
            "----- [{}] -----",
            Section::get_section(section_name).as_str()
        )
        .blue()
    );
    run_tasks(tasks, config, opts, section_name)
}

pub fn run_block(
    block_name: &str,
    config: &Config,
    opts: &mut RunOptions,
) -> Result<(), RunnerError> {
    info!("{}", format!("--- [Block: {}] ---", block_name).magenta());
    if let Some(tasks) = config.blocks.get(block_name) {
        match &tasks.steps {
            Some(steps) => run_tasks(steps, config, opts, block_name),
            None => Ok(()),
        }
    } else {
        Err(RunnerError::CmdFailed(format!(
            "Block '{}' not found",
            block_name
        )))
    }
}

pub fn run_tasks(
    tasks: &Vec<String>,
    config: &Config,
    opts: &mut RunOptions,
    parent_name: &str,
) -> Result<(), RunnerError> {
    let order = tasks;

    let mut failures: Vec<String> = Vec::new();

    for task in order {
        if task.trim().is_empty() {
            continue;
        }
        if task.split(" ").collect::<Vec<&str>>().len() == 1
            && !task.starts_with("'")
            && !task.starts_with("\"")
        {
            // This is a block reference
            let block_name = task.trim();
            if config.blocks.contains_key(block_name) {
                match run_block(block_name, config, opts) {
                    Ok(_) => {}
                    Err(e) => {
                        let msg = format!(
                            "Block '{}' execution failed in parent '{}': {}",
                            block_name, parent_name, e
                        );
                        if opts.continue_on_error {
                            warn!("{}", msg);
                            failures.push(msg);
                        } else {
                            return Err(RunnerError::CmdFailed(msg));
                        }
                    }
                }
            }
        }
        info!("{} {}", "$".cyan(), task.cyan());
        if opts.dry_run {
            continue;
        }
        match run_shell(task, opts) {
            Ok(status) if status.success() => {}
            Ok(status) => {
                let msg = format!(
                    "Parent '{}' command failed: '{}' (exit {:?})",
                    parent_name,
                    task,
                    status.code()
                );
                if opts.continue_on_error {
                    warn!("{}", msg);
                    failures.push(msg);
                } else {
                    return Err(RunnerError::CmdFailed(msg));
                }
            }
            Err(e) => {
                let msg = format!(
                    "Parent '{}' command spawn error: '{}' -> {}",
                    parent_name, task, e
                );
                if opts.continue_on_error {
                    warn!("{}", msg);
                    failures.push(msg);
                } else {
                    return Err(RunnerError::CmdFailed(msg));
                }
            }
        }
    }

    if !failures.is_empty() {
        let joined = failures.join("\n");
        return Err(RunnerError::CmdFailed(joined));
    }

    Ok(())
}

fn run_shell(cmdline: &str, opts: &mut RunOptions) -> Result<ExitStatus, RunnerError> {
    let mut cmd = if opts.os == "windows" {
        let mut c = Command::new("cmd");
        c.arg("/C")
            .arg(cmdline.to_string() + "; set > .env.vars.zbuild");
        c.env("TERM", "xterm-256color");
        c.env("ANSICON", "1");
        c
    } else {
        let mut c = Command::new("sh");
        c.arg("-c")
            .arg(cmdline.to_string() + "; env > .env.vars.zbuild");
        c.env("TERM", "xterm-256color");
        c
    };

    if let Some(ref dir) = opts.cwd {
        cmd.current_dir(dir);
    }
    for (k, v) in &opts.extra_env {
        cmd.env(k, v);
    }

    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;

    let status = child.wait()?;

    // Read .env.vars from previous command if exists
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

    // Clean up .env.vars after reading
    if env_vars_path.exists() {
        let _ = std::fs::remove_file(&env_vars_path);
    }

    Ok(status)
}
