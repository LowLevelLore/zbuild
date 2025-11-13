use crate::config_model::{OPERATING_SYSTEMS, SECTIONS};
use crate::{config_model::Config, error::RunnerError};
use std::collections::HashMap;

pub fn parse_config_yaml(yaml: &str) -> Result<Config, RunnerError> {
    let cfg: Config = serde_yaml::from_str(yaml)?;
    Ok(cfg)
}

pub fn parse_kv(s: &str) -> Result<(String, String), String> {
    let (k, v) = s
        .split_once('=')
        .ok_or_else(|| "expected KEY=VALUE".to_string())?;
    if k.is_empty() {
        return Err("key cannot be empty".into());
    }
    Ok((k.to_string(), v.to_string()))
}

fn validate_config(_config: &Config) -> Result<(), RunnerError> {
    _config.blocks.iter().try_for_each(|(block_name, _)| {
        if SECTIONS.contains(&block_name.as_str()) {
            return Err(RunnerError::Constraints(format!(
                "Block name '{block_name}' conflicts with reserved section name"
            )));
        }

        if OPERATING_SYSTEMS.contains(&block_name.as_str()) {
            return Err(RunnerError::Constraints(format!(
                "Block name '{block_name}' conflicts with reserved operating system name"
            )));
        }

        Ok(())
    })?;
    Ok(())
}

pub fn parse_yaml(yaml: &str) -> Result<Config, RunnerError> {
    let config = parse_config_yaml(yaml);
    match config {
        Ok(cfg) => match validate_config(&cfg) {
            Ok(_) => Ok(cfg),
            Err(e) => Err(e),
        },
        Err(e) => Err(RunnerError::CmdFailed(format!(
            "failed to parse YAML config: {e}"
        ))),
    }
}

#[allow(dead_code)]
fn parse_env_dump(content: &str) -> HashMap<String, String> {
    let mut env_map = HashMap::new();

    for entry in content.split('\0') {
        if entry.is_empty() {
            continue;
        }

        if let Some(equal_pos) = entry.find('=') {
            let key = &entry[..equal_pos];
            let value = &entry[equal_pos + 1..];

            env_map.insert(key.to_string(), value.to_string());
        }
    }

    env_map
}
