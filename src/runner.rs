use log::{error, info, warn};
use std::{
    path::PathBuf,
    process::{Command, ExitStatus, Stdio},
};

use colored::Colorize;

use crate::{
    config_model::{Config, ExecutionPolicy, PlatformCommands},
    environment::{EnvVariableSource, Environment},
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

    pub fn map_section(yml_name: &str) -> &str {
        match yml_name {
            "prebuild" => "PreBuild",
            "build" => "Build",
            "postbuild" => "PostBuild",
            "test" => "Test",
            "predeploy" => "PreDeploy",
            "deploy" => "Deploy",
            "postdeploy" => "PostDeploy",
            "clean" => "Clean",
            _ => "Unknown",
        }
    }

    pub fn get_section(name: &str) -> Section {
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

fn commands_for_os<'a>(
    pc: &'a PlatformCommands,
    env: &mut Environment<'a>,
    os: &str,
) -> Option<&'a Vec<String>> {
    match os {
        "windows" => {
            if pc.windows.is_some() {
                let current = pc.windows.as_ref().unwrap();
                if current.local_config.is_some() {
                    if let Some(env_vars) = &current.local_config.as_ref().unwrap().env {
                        for (key, value) in env_vars {
                            env.upsert_variable(
                                key.to_string(),
                                value.to_string(),
                                EnvVariableSource::Local,
                            );
                        }
                    }
                    if let Some(exec_policy) =
                        &current.local_config.as_ref().unwrap().execution_policy
                    {
                        env.execution_policy = exec_policy.clone();
                    }
                }
                current.steps.as_ref()
            } else {
                None
            }
        }
        "linux" => {
            if pc.linux.is_some() {
                let current = pc.linux.as_ref().unwrap();
                if current.local_config.is_some() {
                    if let Some(env_vars) = &current.local_config.as_ref().unwrap().env {
                        for (key, value) in env_vars {
                            env.upsert_variable(
                                key.to_string(),
                                value.to_string(),
                                EnvVariableSource::Local,
                            );
                        }
                    }
                    if let Some(exec_policy) =
                        &current.local_config.as_ref().unwrap().execution_policy
                    {
                        env.execution_policy = exec_policy.clone();
                    }
                }
                current.steps.as_ref()
            } else {
                None
            }
        }
        "macos" => {
            if pc.macos.is_some() {
                let current = pc.macos.as_ref().unwrap();
                if current.local_config.is_some() {
                    if let Some(env_vars) = &current.local_config.as_ref().unwrap().env {
                        for (key, value) in env_vars {
                            env.upsert_variable(
                                key.to_string(),
                                value.to_string(),
                                EnvVariableSource::Local,
                            );
                        }
                    }
                    if let Some(exec_policy) =
                        &current.local_config.as_ref().unwrap().execution_policy
                    {
                        env.execution_policy = exec_policy.clone();
                    }
                }
                current.steps.as_ref()
            } else {
                None
            }
        }
        _ => None,
    }
}

pub fn run(config: &Config, env: &mut Environment) -> Result<(), RunnerError> {
    let filter: Option<std::collections::HashSet<&'static str>> = env
        .sections
        .as_ref()
        .map(|v| v.iter().map(|s| s.as_str()).collect());
    for (section_name, commands) in config.tasks.ordered_sections() {
        if let Some(ref filt) = filter {
            if !filt.contains(&section_name) {
                continue;
            }
        } else if section_name == Section::Clean.as_str()
            && env.banned_sections.is_some()
            && env
                .banned_sections
                .as_ref()
                .unwrap()
                .contains(&Section::get_section(section_name))
        {
            continue;
        }
        match commands {
            Some(c) => {
                let mut section_environment = env.clone();
                let result = match commands_for_os(c, &mut section_environment, env.os) {
                    Some(cmds) => run_section(section_name, config, cmds, &section_environment),
                    None => {
                        continue;
                    }
                };
                match result {
                    Ok(new_env) => {
                        env.merge_env(new_env);
                    }
                    Err(e) => {
                        if section_environment.execution_policy == ExecutionPolicy::CarryFroward {
                            warn!(
                                "{}",
                                format!(
                                    "Section '{}' failed, carrying forward because global execution policy is CarryForward",
                                    section_name,
                                )
                                .to_string()
                                .yellow()
                            );
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
    Ok(())
}

pub fn run_section<'a>(
    section_name: &str,
    config: &Config,
    tasks: &Vec<String>,
    env: &'a Environment,
) -> Result<Environment<'a>, RunnerError> {
    info!(
        "{}",
        format!(
            "----- [{}] -----",
            Section::get_section(section_name).as_str()
        )
        .blue()
    );
    run_tasks(tasks, config, env, section_name)
}

pub fn run_block<'a>(
    block_name: &str,
    config: &Config,
    env: &'a Environment,
) -> Result<Environment<'a>, RunnerError> {
    info!("{}", format!("--- [Block: {}] ---", block_name).magenta());
    let mut block_environment = env.clone();
    if let Some(tasks) = config.blocks.get(block_name) {
        match &tasks.steps {
            Some(steps) => {
                if config
                    .blocks
                    .get(block_name)
                    .unwrap()
                    .local_config
                    .is_some()
                {
                    if let Some(env_vars) = &config
                        .blocks
                        .get(block_name)
                        .unwrap()
                        .local_config
                        .as_ref()
                        .unwrap()
                        .env
                    {
                        for (key, value) in env_vars {
                            block_environment.upsert_variable(
                                key.to_string(),
                                value.to_string(),
                                EnvVariableSource::Local,
                            );
                        }
                    }
                    if let Some(exec_policy) = &config
                        .blocks
                        .get(block_name)
                        .unwrap()
                        .local_config
                        .as_ref()
                        .unwrap()
                        .execution_policy
                    {
                        block_environment.execution_policy = exec_policy.clone();
                    }
                }

                let current_environment = block_environment.clone();

                let res = run_tasks(steps, config, &current_environment, block_name);
                let out: Result<Environment, RunnerError> = match res {
                    Ok(new_env) => {
                        block_environment.merge_env(new_env);
                        return Ok(block_environment);
                    }
                    Err(e) => Err(e),
                };

                if out.is_err() {
                    if env.execution_policy == ExecutionPolicy::CarryFroward {
                        let internal_error = out.err().unwrap().to_string().yellow();
                        warn!("{}", internal_error);
                        warn!("{}", format!("Block '{}' failed silently, moving forward because the parent execution policy is CarryForward", block_name).yellow());
                        Ok(block_environment)
                    } else {
                        let internal_error = out.as_ref().err().unwrap().to_string().red();
                        error!("{}", internal_error);
                        out
                    }
                } else {
                    out
                }
            }
            None => Ok(block_environment),
        }
    } else {
        Err(RunnerError::CmdFailed(format!(
            "Block '{}' not found",
            block_name
        )))
    }
}

pub fn run_tasks<'a>(
    tasks: &Vec<String>,
    config: &Config,
    env: &'a Environment,
    parent_name: &str,
) -> Result<Environment<'a>, RunnerError> {
    let order = tasks;
    let mut new_env = env.clone();

    for task in order {
        info!("{} {}", "$".cyan(), task.cyan());

        if env.dry_run || task.trim().is_empty() {
            continue;
        }

        let task = task.trim();
        let is_block = task.split(' ').count() == 1
            && !task.starts_with('\'')
            && !task.starts_with('"')
            && config.blocks.contains_key(task);
        if is_block {
            let block_name = task.trim();
            if config.blocks.contains_key(block_name) {
                let current_environment = new_env.clone();
                match run_block(block_name, config, &current_environment) {
                    Ok(result_env) => {
                        new_env.merge_env(result_env);
                    }
                    Err(_) => {
                        let msg = format!(
                            "Block '{}' execution failed in parent '{}'",
                            block_name, parent_name
                        );
                        if env.execution_policy == ExecutionPolicy::CarryFroward {
                            warn!("{}", msg.yellow());
                            // failures.push(msg);
                        } else {
                            return Err(RunnerError::CmdFailed(msg));
                        }
                    }
                }
            }
        } else {
            let current_environment = new_env.clone();
            let res: Result<(ExitStatus, Environment), RunnerError> =
                run_shell(task, &current_environment);
            match res {
                Ok((status, result_env)) => {
                    if status.success() {
                        new_env.merge_env(result_env);
                    } else {
                        let msg = format!(
                            "Parent '{}' command failed: '{}' (exit {:?})",
                            parent_name,
                            task,
                            status.code()
                        );
                        if env.execution_policy == ExecutionPolicy::CarryFroward {
                            warn!("{}", msg.yellow());
                            // failures.push(msg);
                        } else {
                            return Err(RunnerError::CmdFailed(msg));
                        }
                    }
                }
                Err(e) => {
                    let msg = format!(
                        "Parent '{}' command spawn error: '{}' -> {}",
                        parent_name, task, e
                    );
                    if env.execution_policy == ExecutionPolicy::CarryFroward {
                        warn!("{}", msg.yellow());
                        // failures.push(msg);
                    } else {
                        return Err(RunnerError::CmdFailed(msg));
                    }
                }
            }
        }
    }

    Ok(new_env)
}

fn run_shell<'a>(
    cmdline: &str,
    env: &'a Environment,
) -> Result<(ExitStatus, Environment<'a>), RunnerError> {
    let mut cmd = if env.os == "windows" {
        let mut c = Command::new("cmd");
        c.arg("/C")
            .arg(cmdline.to_string() + "&& set > .env.vars.zbuild");
        c.env("TERM", "xterm-256color");
        c.env("ANSICON", "1");
        c
    } else {
        let mut c = Command::new("sh");
        c.arg("-c")
            .arg(cmdline.to_string() + "&& env > .env.vars.zbuild");
        c.env("TERM", "xterm-256color");
        c
    };

    if let Some(ref dir) = env.cwd {
        cmd.current_dir(dir);
    }
    for (k, v) in &env.variables {
        cmd.env(k, v.value.clone());
    }

    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;

    let status = child.wait()?;

    // Read .env.vars from previous command if exists
    let env_vars_path = if let Some(ref dir) = env.cwd {
        dir.join(".env.vars.zbuild")
    } else {
        PathBuf::from(".env.vars.zbuild")
    };

    let mut new_environment = env.clone();

    if env_vars_path.exists()
        && let Ok(content) = std::fs::read_to_string(&env_vars_path)
    {
        new_environment.load_env(content, EnvVariableSource::Script);
    }

    // Clean up .env.vars after reading
    if env_vars_path.exists() {
        let _ = std::fs::remove_file(&env_vars_path);
    }

    Ok((status, new_environment))
}
