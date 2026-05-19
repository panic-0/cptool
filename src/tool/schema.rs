use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

pub(crate) const DEFAULT_TIME_LIMIT_SECS: f64 = 1.0;
pub(crate) const DEFAULT_MEMORY_LIMIT_MB: f64 = 512.0;
pub(crate) const DEFAULT_OUTPUT_LIMIT_BYTES: usize = 33_554_432;
#[derive(Clone, Debug)]
pub struct CaseSelector {
    pub bundle: String,
    pub index: usize,
}

#[derive(Clone, Debug)]
pub struct RunOptions {
    pub work_dir: PathBuf,
    pub program: Option<String>,
    pub source: Option<PathBuf>,
    pub selector: Option<String>,
    pub stdin_text: Option<String>,
    pub stdin_path: Option<PathBuf>,
    pub stdout_path: Option<PathBuf>,
    pub stderr_path: Option<PathBuf>,
    pub args: Vec<String>,
    pub output_limit_bytes: usize,
}

#[derive(Clone, Debug)]
pub struct RunResult {
    pub label: String,
    pub ok: bool,
    pub kind: String,
    pub exit_code: Option<i32>,
    pub elapsed_ms: u128,
    pub stdout_bytes: Vec<u8>,
    pub stderr_bytes: Vec<u8>,
    pub stdout: String,
    pub stderr: String,
    pub truncated_stdout: bool,
    pub truncated_stderr: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommandProgram {
    pub path: PathBuf,
    #[serde(default)]
    pub extra_args: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CppProgram {
    pub path: PathBuf,
    #[serde(default = "default_compile_args")]
    pub compile_args: Vec<String>,
}

fn default_compile_args() -> Vec<String> {
    vec![
        "-O2".to_string(),
        "-std=c++20".to_string(),
        "-Wall".to_string(),
        "-Wextra".to_string(),
        "-pedantic".to_string(),
    ]
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ProgramInfo {
    #[serde(rename = "command")]
    Command(CommandProgram),
    #[serde(rename = "cpp")]
    Cpp(CppProgram),
    #[serde(rename = "python")]
    Python(CommandProgram),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Program {
    pub info: ProgramInfo,
    pub time_limit_secs: f64,
    pub memory_limit_mb: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TestCase {
    #[serde(rename = "generator")]
    pub generator_name: String,
    pub args: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TestBundle {
    pub cases: Vec<TestCase>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum TestTaskType {
    #[serde(rename = "sum")]
    Sum,
    #[serde(rename = "min")]
    Min,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TestTask {
    pub name: String,
    pub score: f64,
    #[serde(rename = "type")]
    pub task_type: TestTaskType,
    pub bundles: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Test {
    pub bundles: HashMap<String, TestBundle>,
    pub tasks: Vec<TestTask>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Problem {
    pub name: String,
    pub programs: HashMap<String, Program>,
    pub test: Test,
    #[serde(rename = "solution")]
    pub solution_name: String,
    #[serde(rename = "validator")]
    pub validator_name: Option<String>,
    #[serde(rename = "checker")]
    pub checker_name: Option<String>,
}
