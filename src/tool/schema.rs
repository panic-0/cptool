use serde::{Deserialize, Deserializer, Serialize, de};
use serde_yml::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

pub(crate) const DEFAULT_TIME_LIMIT_SECS: f64 = 1.0;
pub(crate) const DEFAULT_MEMORY_LIMIT_MB: f64 = 512.0;
pub const DEFAULT_OUTPUT_LIMIT_BYTES: usize = 33_554_432;
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
    pub generation_lock_timeout: Option<Duration>,
}

#[derive(Clone, Debug)]
pub struct RunResult {
    pub label: String,
    pub ok: bool,
    pub kind: String,
    pub exit_code: Option<i32>,
    pub diagnostic: Option<String>,
    pub elapsed_ms: u128,
    pub stdout_bytes: Vec<u8>,
    pub stderr_bytes: Vec<u8>,
    pub stdout: String,
    pub stderr: String,
    pub truncated_stdout: bool,
    pub truncated_stderr: bool,
}

impl RunResult {
    pub fn status_line(&self) -> String {
        let mut line = format!(
            "{}: {} exit={} elapsed={}ms",
            self.label,
            self.kind,
            self.exit_code
                .map(|code| code.to_string())
                .unwrap_or_else(|| "none".to_string()),
            self.elapsed_ms
        );
        if self.truncated_stdout {
            line.push_str(" stdout=truncated");
        }
        if self.truncated_stderr {
            line.push_str(" stderr=truncated");
        }
        line
    }

    pub fn failure_report(&self, context: &str) -> String {
        let mut report = format!("{context}: {}", self.status_line());
        if let Some(diagnostic) = &self.diagnostic {
            report.push_str("\ndiagnostic:\n");
            report.push_str(diagnostic);
        }
        if !self.stderr.is_empty() {
            report.push_str("\nstderr:\n");
            report.push_str(&self.stderr);
        } else if !self.stdout.is_empty() {
            report.push_str("\nstdout:\n");
            report.push_str(&self.stdout);
        }
        report
    }

    pub fn summary_line(&self) -> String {
        format!(
            "{} stdout_bytes={} stdout_lines={} stdout_sha256={} stderr_bytes={} stderr_nonempty={}",
            self.status_line(),
            self.stdout_bytes.len(),
            count_lines(&self.stdout_bytes),
            sha256_hex(&self.stdout_bytes),
            self.stderr_bytes.len(),
            !self.stderr_bytes.is_empty(),
        )
    }
}

fn count_lines(bytes: &[u8]) -> usize {
    if bytes.is_empty() {
        0
    } else {
        bytes.iter().filter(|byte| **byte == b'\n').count() + usize::from(!bytes.ends_with(b"\n"))
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
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

pub(crate) fn default_compile_args() -> Vec<String> {
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

#[derive(Clone, Debug, Serialize)]
pub struct TestCase {
    #[serde(rename = "generator")]
    pub generator_name: String,
    pub args: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TestBundle {
    pub cases: Vec<TestCase>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Serialize)]
pub struct Test {
    pub bundles: HashMap<String, TestBundle>,
    pub tasks: Vec<TestTask>,
}

#[derive(Deserialize)]
struct RawTest {
    #[serde(default)]
    generator: Option<String>,
    #[serde(default, rename = "type")]
    task_type: Option<TestTaskType>,
    bundles: HashMap<String, RawTestBundle>,
    tasks: Vec<RawTestTask>,
}

#[derive(Deserialize)]
struct RawTestBundle {
    #[serde(default)]
    generator: Option<String>,
    cases: Vec<RawTestCase>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawTestCase {
    Args(Vec<Value>),
    Full {
        #[serde(default, rename = "generator")]
        generator_name: Option<String>,
        #[serde(default)]
        args: Vec<Value>,
    },
}

#[derive(Deserialize)]
struct RawTestTask {
    name: String,
    score: f64,
    #[serde(default, rename = "type")]
    task_type: Option<TestTaskType>,
    bundles: Vec<String>,
    #[serde(default)]
    dependencies: Vec<String>,
}

impl<'de> Deserialize<'de> for Test {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawTest::deserialize(deserializer)?;
        let global_generator = raw.generator;
        let global_task_type = raw.task_type;
        let bundles = raw
            .bundles
            .into_iter()
            .map(|(bundle_name, bundle)| {
                let bundle_generator = bundle.generator.or_else(|| global_generator.clone());
                let cases = bundle
                    .cases
                    .into_iter()
                    .enumerate()
                    .map(|(case_index, case)| {
                        normalize_test_case(
                            case,
                            bundle_generator.as_deref(),
                            &bundle_name,
                            case_index,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok((bundle_name, TestBundle { cases }))
            })
            .collect::<Result<HashMap<_, _>, D::Error>>()?;
        let tasks = raw
            .tasks
            .into_iter()
            .enumerate()
            .map(|(task_index, task)| normalize_test_task(task, global_task_type, task_index))
            .collect::<Result<Vec<_>, D::Error>>()?;

        Ok(Self { bundles, tasks })
    }
}

fn normalize_test_task<E>(
    task: RawTestTask,
    default_task_type: Option<TestTaskType>,
    task_index: usize,
) -> Result<TestTask, E>
where
    E: de::Error,
{
    let task_type = task.task_type.or(default_task_type).ok_or_else(|| {
        E::custom(format!(
            "test.tasks[{task_index}] is missing `type` and no default type is declared for test"
        ))
    })?;
    Ok(TestTask {
        name: task.name,
        score: task.score,
        task_type,
        bundles: task.bundles,
        dependencies: task.dependencies,
    })
}

fn normalize_test_case<E>(
    case: RawTestCase,
    default_generator: Option<&str>,
    bundle_name: &str,
    case_index: usize,
) -> Result<TestCase, E>
where
    E: de::Error,
{
    match case {
        RawTestCase::Args(args) => {
            let Some(generator_name) = default_generator else {
                return Err(E::custom(format!(
                    "test.bundles.{bundle_name}.cases[{case_index}] uses args-only shorthand but no generator is declared for the case, bundle, or test"
                )));
            };
            let args = raw_args_to_strings(args, bundle_name, case_index)?;
            Ok(TestCase {
                generator_name: generator_name.to_string(),
                args,
            })
        }
        RawTestCase::Full {
            generator_name,
            args,
        } => {
            let generator_name = generator_name
                .or_else(|| default_generator.map(str::to_string))
                .ok_or_else(|| {
                    E::custom(format!(
                        "test.bundles.{bundle_name}.cases[{case_index}] is missing `generator` and no default generator is declared for the bundle or test"
                    ))
                })?;
            let args = raw_args_to_strings(args, bundle_name, case_index)?;
            Ok(TestCase {
                generator_name,
                args,
            })
        }
    }
}

fn raw_args_to_strings<E>(
    args: Vec<Value>,
    bundle_name: &str,
    case_index: usize,
) -> Result<Vec<String>, E>
where
    E: de::Error,
{
    args.into_iter()
        .enumerate()
        .map(|(arg_index, value)| {
            raw_arg_to_string(value).ok_or_else(|| {
                E::custom(format!(
                    "test.bundles.{bundle_name}.cases[{case_index}].args[{arg_index}] must be a string, number, boolean, or null"
                ))
            })
        })
        .collect()
}

fn raw_arg_to_string(value: Value) -> Option<String> {
    match value {
        Value::Null => Some("null".to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Number(value) => Some(value.to_string()),
        Value::String(value) => Some(value),
        Value::Tagged(tagged) => raw_arg_to_string(tagged.value),
        Value::Sequence(_) | Value::Mapping(_) => None,
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Stress {
    #[serde(default)]
    pub plans: Vec<StressPlan>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StressPlan {
    pub name: String,
    pub generator: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub against: Vec<String>,
    #[serde(default = "default_stress_cases")]
    pub cases: usize,
    #[serde(default)]
    pub seed_base: Option<u64>,
    #[serde(default)]
    pub expect: StressPlanExpectation,
}

fn default_stress_cases() -> usize {
    100
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum StressPlanExpectation {
    #[default]
    #[serde(rename = "pass")]
    Pass,
    #[serde(rename = "fail")]
    Fail,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Problem {
    pub name: String,
    #[serde(default)]
    pub output: OutputConfig,
    #[serde(default)]
    pub stress: Stress,
    pub programs: HashMap<String, Program>,
    pub test: Test,
    #[serde(rename = "solution")]
    pub solution_name: String,
    #[serde(rename = "validator")]
    pub validator_name: Option<String>,
    #[serde(default)]
    pub validator_omitted_reason: Option<String>,
    #[serde(rename = "checker")]
    pub checker_name: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct OutputConfig {
    #[serde(default)]
    pub allow_empty: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_summary_reports_sizes_lines_and_hash() {
        let result = RunResult {
            label: "std".to_string(),
            ok: true,
            kind: "ok".to_string(),
            exit_code: Some(0),
            diagnostic: None,
            elapsed_ms: 12,
            stdout_bytes: b"a\nb".to_vec(),
            stderr_bytes: b"warn".to_vec(),
            stdout: "a\nb".to_string(),
            stderr: "warn".to_string(),
            truncated_stdout: false,
            truncated_stderr: false,
        };

        assert_eq!(
            result.summary_line(),
            "std: ok exit=0 elapsed=12ms stdout_bytes=3 stdout_lines=2 stdout_sha256=7e18f737311b2dc3b2f269dd78396b0351f14fb66efa879f768cb23181883c78 stderr_bytes=4 stderr_nonempty=true"
        );
    }

    #[test]
    fn run_summary_counts_empty_output_as_zero_lines() {
        let result = RunResult {
            label: "std".to_string(),
            ok: true,
            kind: "ok".to_string(),
            exit_code: Some(0),
            diagnostic: None,
            elapsed_ms: 1,
            stdout_bytes: Vec::new(),
            stderr_bytes: Vec::new(),
            stdout: String::new(),
            stderr: String::new(),
            truncated_stdout: false,
            truncated_stderr: false,
        };

        assert!(result.summary_line().contains("stdout_lines=0"));
        assert!(result.summary_line().contains("stderr_nonempty=false"));
    }

    #[test]
    fn failure_report_includes_optional_diagnostic_only_on_failure_path() {
        let result = RunResult {
            label: "std".to_string(),
            ok: false,
            kind: "runtime_error".to_string(),
            exit_code: Some(-1073741819),
            diagnostic: Some("hint: access violation".to_string()),
            elapsed_ms: 1,
            stdout_bytes: Vec::new(),
            stderr_bytes: Vec::new(),
            stdout: String::new(),
            stderr: String::new(),
            truncated_stdout: false,
            truncated_stderr: false,
        };

        assert_eq!(
            result.status_line(),
            "std: runtime_error exit=-1073741819 elapsed=1ms"
        );
        assert_eq!(
            result.failure_report("solution failed"),
            "solution failed: std: runtime_error exit=-1073741819 elapsed=1ms\ndiagnostic:\nhint: access violation"
        );
    }

    #[test]
    fn test_cases_accept_global_bundle_and_case_generators() {
        let test: Test = serde_yml::from_str(
            r#"
generator: gen
type: min
bundles:
  global:
    cases:
    - [1, 2]
    - args: [3, 4]
  local:
    generator: local_gen
    cases:
    - [5]
    - args: [6]
  explicit:
    generator: ignored
    cases:
    - generator: special_gen
      args: [7]
tasks:
- name: main
  score: 100.0
  bundles: [global, local, explicit]
"#,
        )
        .unwrap();

        assert_eq!(test.bundles["global"].cases[0].generator_name, "gen");
        assert_eq!(test.bundles["global"].cases[0].args, ["1", "2"]);
        assert_eq!(test.bundles["global"].cases[1].generator_name, "gen");
        assert_eq!(test.bundles["local"].cases[0].generator_name, "local_gen");
        assert_eq!(test.bundles["local"].cases[1].generator_name, "local_gen");
        assert_eq!(
            test.bundles["explicit"].cases[0].generator_name,
            "special_gen"
        );
        assert_eq!(test.tasks[0].task_type, TestTaskType::Min);
    }

    #[test]
    fn tasks_accept_global_type_and_task_override() {
        let test: Test = serde_yml::from_str(
            r#"
generator: gen
type: min
bundles:
  main:
    cases:
    - []
tasks:
- name: base
  score: 40.0
  bundles: [main]
- name: bonus
  score: 60.0
  type: sum
  bundles: [main]
  dependencies: [base]
"#,
        )
        .unwrap();

        assert_eq!(test.tasks[0].task_type, TestTaskType::Min);
        assert_eq!(test.tasks[1].task_type, TestTaskType::Sum);
    }

    #[test]
    fn task_type_requires_global_or_task_value() {
        let err = serde_yml::from_str::<Test>(
            r#"
generator: gen
bundles:
  sample:
    cases:
    - []
tasks:
- name: sample
  score: 100.0
  bundles: [sample]
"#,
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("missing `type`"));
    }

    #[test]
    fn args_only_test_case_requires_default_generator() {
        let err = serde_yml::from_str::<Test>(
            r#"
bundles:
  sample:
    cases:
    - [1]
tasks:
- name: sample
  score: 100.0
  type: min
  bundles: [sample]
"#,
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("args-only shorthand"));
    }
}
