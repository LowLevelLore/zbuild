use crate::config_model::{OPERATING_SYSTEMS, SECTIONS};
use crate::{config_model::Config, error::RunnerError};

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
    _config.blocks.iter().try_for_each(|(block_name, steps)| {
        if SECTIONS.contains(&block_name.as_str()) {
            return Err(RunnerError::Constraints(format!(
                "Block name '{}' conflicts with reserved section name",
                block_name
            )));
        }

        if OPERATING_SYSTEMS.contains(&block_name.as_str()) {
            return Err(RunnerError::Constraints(format!(
                "Block name '{}' conflicts with reserved operating system name",
                block_name
            )));
        }

        if let Some(step_list) = &steps.steps {
            for step in step_list {
                if step.trim().is_empty() {
                    return Err(RunnerError::Constraints(format!(
                        "Empty step found in block '{}'",
                        block_name
                    )));
                }
            }
        }
        Ok(())
    })?;
    Ok(())
}

pub fn parse_yaml(yaml: &str) -> Result<Config, RunnerError> {
    let config = parse_config_yaml(yaml);
    // println!("{:?}", config);
    match config {
        Ok(cfg) => match validate_config(&cfg) {
            Ok(_) => Ok(cfg),
            Err(e) => Err(e),
        },
        Err(e) => Err(RunnerError::CmdFailed(format!(
            "failed to parse YAML config: {}",
            e
        ))),
    }
}
