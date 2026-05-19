use super::Exporter;
use crate::tool::{ProgramInfo, TestTaskType, resolve_path};
use anyhow::Result;
use serde::{Deserialize, Serialize};

// https://github.com/syzoj/syzoj/blob/573796fa7670e28d428692f1d91e7ea50ee154e5/utility.js#L192

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum ProgramType {
    // TODO: cpp11, cpp14, cpp17
    #[serde(rename = "cpp")]
    Cpp,
    // TODO: add more
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Program {
    pub language: ProgramType,
    #[serde(rename = "fileName")]
    pub file_name: String,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum SubtaskType {
    #[serde(rename = "sum")]
    Sum,
    #[serde(rename = "min")]
    Min,
    #[serde(rename = "mul")]
    Mul,
}

impl From<TestTaskType> for SubtaskType {
    fn from(task_type: TestTaskType) -> Self {
        match task_type {
            TestTaskType::Sum => SubtaskType::Sum,
            TestTaskType::Min => SubtaskType::Min,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Subtask {
    #[serde(rename = "type")]
    pub subtask_type: SubtaskType,
    pub score: f64,
    pub cases: Vec<String>,
    pub dependencies: Option<Vec<usize>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Problem {
    #[serde(rename = "inputFile")]
    pub input_file: Option<String>,
    #[serde(rename = "outputFile")]
    pub output_file: Option<String>,
    #[serde(rename = "userOutput")]
    pub answer_file: Option<String>,

    pub subtasks: Vec<Subtask>,

    #[serde(rename = "specialJudge")]
    pub special_judge: Option<Program>,
}

pub struct SyzojExporter;

impl Exporter for SyzojExporter {
    fn export(
        problem: &crate::tool::Problem,
        work_dir: &std::path::Path,
        data_dir: &std::path::Path,
        export_dir: &std::path::Path,
    ) -> Result<()> {
        use std::collections::HashMap;
        let mut task_id = HashMap::new();
        problem.test.tasks.iter().enumerate().for_each(|(i, task)| {
            task_id.insert(&task.name, i);
        });

        let mut counter = 0usize;
        let subtasks = problem
            .test
            .tasks
            .iter()
            .map(|task| {
                let cases = task
                    .bundles
                    .iter()
                    .map(|bundle_name| {
                        let bundle =
                            problem.test.bundles.get(bundle_name).ok_or_else(|| {
                                anyhow::anyhow!("bundle `{bundle_name}` not found")
                            })?;
                        bundle
                            .cases
                            .iter()
                            .enumerate()
                            .map(|(case_index, _case)| {
                                let name = format!("{counter}");
                                let input_path = export_dir.join(format!("{name}.in"));
                                let answer_path = export_dir.join(format!("{name}.ans"));
                                let case_name = format!("{bundle_name}-{case_index}");
                                counter += 1;
                                std::fs::copy(
                                    data_dir.join(format!("{case_name}.in")),
                                    input_path,
                                )?;
                                std::fs::copy(
                                    data_dir.join(format!("{case_name}.ans")),
                                    answer_path,
                                )?;
                                Ok(name)
                            })
                            .collect::<Result<Vec<_>>>()
                    })
                    .collect::<Result<Vec<_>>>()?
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>();
                let dependencies = task
                    .dependencies
                    .iter()
                    .map(|task_name| {
                        task_id
                            .get(task_name)
                            .ok_or_else(|| anyhow::anyhow!("task `{task_name}` not found"))
                            .map(|&id| id + 1)
                    })
                    .collect::<Result<Vec<_>>>()?;
                Ok(Subtask {
                    subtask_type: task.task_type.into(),
                    score: task.score,
                    cases,
                    dependencies: Some(dependencies),
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let special_judge = if let Some(checker_name) = &problem.checker_name {
            let checker = problem
                .programs
                .get(checker_name)
                .ok_or_else(|| anyhow::anyhow!("checker `{checker_name}` not found"))?;
            match &checker.info {
                ProgramInfo::Command(_) => Err(anyhow::anyhow!(
                    "command program is not supported as special judge in syzoj exporter"
                )),
                ProgramInfo::Python(_) => Err(anyhow::anyhow!(
                    "python program is not supported as special judge in syzoj exporter"
                )),
                ProgramInfo::Cpp(program) => {
                    let name = "spj.cpp".to_string();
                    let path = export_dir.join(&name);
                    std::fs::copy(resolve_path(work_dir, &program.path), path)?;
                    Ok(Some(Program {
                        language: ProgramType::Cpp,
                        file_name: name,
                    }))
                }
            }?
        } else {
            None
        };

        let result = Problem {
            input_file: Some("#.in".to_string()),
            output_file: Some("#.ans".to_string()),
            answer_file: None,
            subtasks,
            special_judge,
        };

        let yaml = serde_yml::to_string(&result)?;
        std::fs::write(export_dir.join("data.yml"), yaml)?;

        Ok(())
    }
}
