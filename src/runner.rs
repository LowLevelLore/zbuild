use log::{debug, info, warn};
use std::{
    collections::HashMap,
    path::PathBuf,
    process::{Command, ExitStatus, Stdio},
};

use colored::Colorize;

use crate::{
    error::RunnerError,
    task_model::{PlatformCommands, Tasks},
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

pub fn run_tasks(tasks: &Tasks, opts: &RunOptions) -> Result<(), RunnerError> {
    let order = tasks.ordered_sections();
    let filter: Option<std::collections::HashSet<&'static str>> = opts
        .sections
        .as_ref()
        .map(|v| v.iter().map(|s| s.as_str()).collect());
    let mut failures: Vec<String> = Vec::new();

    for (section_name, section_opt) in order {
        if let Some(ref filt) = filter {
            if !filt.contains(&section_name) {
                continue;
            }
        } else if section_name == Section::Clean.as_str() {
            continue;
        }

        if let Some(section) = section_opt {
            let commands = commands_for_os(section, opts.os);

            if commands.is_none() || commands.as_ref().unwrap().is_empty() {
                continue;
            }

            info!(
                "{}",
                format!(
                    "----- [{}] -----",
                    Section::get_section(section_name).as_str()
                )
                .blue()
            );

            if let Some(cmds) = commands {
                for cmd in cmds {
                    info!("{} {}", "$".cyan(), cmd.cyan());
                    if opts.dry_run {
                        continue;
                    }
                    match run_shell(cmd, opts) {
                        Ok(status) if status.success() => {}
                        Ok(status) => {
                            let msg = format!(
                                "section '{}' command failed: '{}' (exit {:?})",
                                section_name,
                                cmd,
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
                                "section '{}' command spawn error: '{}' -> {}",
                                section_name, cmd, e
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
            } else {
                debug!("no commands for {} in {}", opts.os, section_name);
            }
        }
    }

    if !failures.is_empty() {
        let joined = failures.join(
            "
",
        );
        return Err(RunnerError::CmdFailed(joined));
    }

    Ok(())
}

fn commands_for_os<'a>(pc: &'a PlatformCommands, os: &str) -> Option<&'a Vec<String>> {
    match os {
        "windows" => pc.windows.as_ref(),
        "linux" => pc.linux.as_ref(),
        "macos" => pc.macos.as_ref(),
        _ => None,
    }
}

fn run_shell(cmdline: &str, opts: &RunOptions) -> Result<ExitStatus, RunnerError> {
    let mut cmd = if opts.os == "windows" {
        let mut c = Command::new("cmd");
        c.arg("/C").arg(cmdline);
        c.env("TERM", "xterm-256color");
        c.env("ANSICON", "1");
        c
    } else {
        let mut c = Command::new("sh");
        c.arg("-c").arg(cmdline);
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
    Ok(status)
}
