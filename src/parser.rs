use crate::{error::RunnerError, task_model::Tasks};

pub fn parse_tasks_yaml(yaml: &str) -> Result<Tasks, RunnerError> {
    let tasks: Tasks = serde_yaml::from_str(yaml)?;
    Ok(tasks)
}
