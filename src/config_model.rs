use serde::Deserialize;
use std::collections::HashMap;

pub(crate) static SECTIONS: &[&str] = &[
    "prebuild",
    "build",
    "postbuild",
    "test",
    "predeploy",
    "deploy",
    "postdeploy",
    "clean",
];

pub(crate) static OPERATING_SYSTEMS: &[&str] = &["windows", "linux", "macos"];

#[derive(Debug, Deserialize, Default)]
pub struct Tasks {
    #[serde(rename = "prebuild")]
    pub prebuild: Option<PlatformCommands>,
    #[serde(rename = "build")]
    pub build: Option<PlatformCommands>,
    #[serde(rename = "postbuild")]
    pub postbuild: Option<PlatformCommands>,
    #[serde(rename = "test")]
    pub test: Option<PlatformCommands>,
    #[serde(rename = "predeploy")]
    pub predeploy: Option<PlatformCommands>,
    #[serde(rename = "deploy")]
    pub deploy: Option<PlatformCommands>,
    #[serde(rename = "postdeploy")]
    pub postdeploy: Option<PlatformCommands>,
    #[serde(rename = "clean")]
    pub clean: Option<PlatformCommands>,
}

#[derive(Debug, Deserialize, Default)]
pub struct Config {
    pub tasks: Tasks,

    #[serde(default)]
    pub blocks: HashMap<String, Block>,

    #[serde(rename = "config", default)]
    pub global_config: Option<GlobalConfig>,
}

#[derive(Debug, Deserialize, Default, Clone, PartialEq, Eq)]
pub enum ExecutionPolicy {
    #[default]
    #[serde(rename = "fast_fail")]
    FastFail,
    #[serde(rename = "carry_forward")]
    CarryFroward,
}

#[derive(Debug, Deserialize, Default)]
pub struct GlobalConfig {
    #[serde(rename = "execution_policy")]
    pub execution_policy: Option<ExecutionPolicy>,
    #[serde(rename = "env")]
    pub env: Option<HashMap<String, String>>,
    #[serde(rename = "skip_sections")]
    pub banned_sections: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct PlatformCommands {
    pub windows: Option<Block>,
    pub linux: Option<Block>,
    pub macos: Option<Block>,
}

#[derive(Debug, Deserialize, Default)]
pub struct LocalConfig {
    #[serde(rename = "execution_policy")]
    pub execution_policy: Option<ExecutionPolicy>,
    #[serde(rename = "env")]
    pub env: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
pub struct Block {
    pub steps: Option<Vec<String>>,
    #[serde(rename = "config")]
    pub local_config: Option<LocalConfig>,
}

impl Tasks {
    /// Returns the tasks in required execution order.
    pub fn ordered_sections(&self) -> [(&'static str, Option<&PlatformCommands>); 8] {
        [
            ("PreBuild", self.prebuild.as_ref()),
            ("Build", self.build.as_ref()),
            ("PostBuild", self.postbuild.as_ref()),
            ("Test", self.test.as_ref()),
            ("PreDeploy", self.predeploy.as_ref()),
            ("Deploy", self.deploy.as_ref()),
            ("PostDeploy", self.postdeploy.as_ref()),
            ("Clean", self.clean.as_ref()),
        ]
    }
}
