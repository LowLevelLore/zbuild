use crate::{config_model::ExecutionPolicy, error::RunnerError, runner::Section};
use std::{
    collections::HashMap,
    path::PathBuf,
    process::{Command, Stdio},
};

#[derive(Debug, Default, Clone)]
pub struct Environment<'a> {
    pub variables: HashMap<String, EnvVariable>,
    pub os: &'a str,
    pub cwd: Option<PathBuf>,
    pub execution_policy: ExecutionPolicy,
    pub dry_run: bool,
    pub banned_sections: Option<Vec<Section>>,
    pub sections: Option<Vec<Section>>,
}

#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub enum EnvVariableSource {
    #[default]
    Default = 1,
    Global = 2,
    Local = 3,
    Passed = 4,
    Script = 5,
}

impl EnvVariableSource {
    pub fn get_priority(&self) -> i32 {
        match self {
            EnvVariableSource::Default => 1,
            EnvVariableSource::Global => 2,
            EnvVariableSource::Local => 3,
            EnvVariableSource::Passed => 4,
            EnvVariableSource::Script => 5,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct EnvVariable {
    pub(crate) source: EnvVariableSource,
    pub(crate) value: String,
}

impl<'a> Environment<'a> {
    pub fn upsert_variable(
        &mut self,
        key: String,
        value: String,
        source: EnvVariableSource,
    ) -> Option<EnvVariable> {
        if self.variables.contains_key(&key) {
            if self.variables.get(&key).unwrap().source.get_priority() <= source.get_priority() {
                return self.variables.insert(key, EnvVariable { source, value });
            } else {
                return None;
            }
        }
        self.variables.insert(key, EnvVariable { source, value })
    }

    pub fn merge_env(&mut self, other: Environment) {
        let variables = &mut self.variables;

        for (key, origin_value) in other.variables.iter() {
            let new_origin = origin_value.source.clone();
            let new_value = origin_value.value.clone();

            if variables.contains_key(key) {
                let origin = variables.get(key).unwrap().source.clone();
                let value = variables.get(key).unwrap().value.clone();
                if new_origin != origin {
                    if new_origin.get_priority() >= origin.get_priority() {
                        variables.insert(
                            key.to_string(),
                            EnvVariable {
                                source: new_origin,
                                value: new_value.to_string(),
                            },
                        );
                    } else {
                        variables.insert(
                            key.to_string(),
                            EnvVariable {
                                source: origin,
                                value: value.to_string(),
                            },
                        );
                    }
                } else {
                    variables.insert(
                        key.to_string(),
                        EnvVariable {
                            source: new_origin,
                            value: new_value,
                        },
                    );
                }
            } else {
                variables.insert(
                    key.to_string(),
                    EnvVariable {
                        source: new_origin,
                        value: new_value,
                    },
                );
            }
        }
    }

    pub fn capture_default_environment(&mut self) -> Result<(), RunnerError> {
        let mut cmd = if self.os == "windows" {
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

        let env_vars_path = if let Some(ref dir) = self.cwd {
            dir.join(".env.vars.zbuild")
        } else {
            PathBuf::from(".env.vars.zbuild")
        };

        if env_vars_path.exists()
            && let Ok(content) = std::fs::read_to_string(&env_vars_path)
        {
            self.load_env(content, EnvVariableSource::Default);
        }

        if env_vars_path.exists() {
            let _ = std::fs::remove_file(&env_vars_path);
        }

        Ok(())
    }

    pub fn load_env(&mut self, content: String, new_origin: EnvVariableSource) {
        let variables = &mut self.variables;

        let new_priority = new_origin.get_priority();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some((key, new_value)) = line.split_once('=') {
                if variables.get(key).is_some() {
                    let old_origin = variables.get(key).unwrap().source.clone();
                    if new_priority >= old_origin.get_priority() {
                        variables.insert(
                            key.to_string(),
                            EnvVariable {
                                source: new_origin.clone(),
                                value: new_value.to_string(),
                            },
                        );
                    }
                } else {
                    variables.insert(
                        key.to_string(),
                        EnvVariable {
                            source: new_origin.clone(),
                            value: new_value.to_string(),
                        },
                    );
                }
            }
        }
    }
}
