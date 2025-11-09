use serde::Deserialize;

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
pub struct PlatformCommands {
    pub windows: Option<Vec<String>>,
    pub linux: Option<Vec<String>>,
    pub macos: Option<Vec<String>>,
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
