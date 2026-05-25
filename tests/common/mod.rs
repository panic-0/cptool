#![allow(dead_code)]

pub use cptool::test_support::python_available;
use cptool::test_support::temp_suffix;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Duration;

pub const GENERATION_LOCK_RELEASE_DELAY: Duration = Duration::from_secs(2);
pub const GENERATION_LOCK_WAIT_TIMEOUT_SECS: &str = "5";
pub const GENERATION_LOCK_WAIT_TIMEOUT_LOG: &str = "timeout=5s";

pub fn configure_python_problem(problem_dir: &Path) {
    std::fs::write(
        problem_dir.join("problem.yaml"),
        r#"name: flow_problem
programs:
  gen:
    info: !python
      path: ./src/gen.py
    time_limit_secs: 1.0
    memory_limit_mb: 128.0
  std:
    info: !python
      path: ./src/solve.py
    time_limit_secs: 1.0
    memory_limit_mb: 128.0
  brute:
    info: !python
      path: ./src/solve.py
    time_limit_secs: 1.0
    memory_limit_mb: 128.0
solution: std
test:
  generator: gen
  bundles:
    sample:
      cases:
      - ["3", "4"]
  tasks:
  - name: sample
    score: 100.0
    type: min
    bundles: [sample]
    pass: [brute]
"#,
    )
    .unwrap();
    std::fs::write(
        problem_dir.join("src").join("gen.py"),
        r#"import sys

a = sys.argv[1] if len(sys.argv) > 1 else "3"
b = sys.argv[2] if len(sys.argv) > 2 else "4"
sys.stdout.buffer.write(f"{a} {b}\n".encode("ascii"))
"#,
    )
    .unwrap();
    std::fs::write(
        problem_dir.join("src").join("solve.py"),
        r#"import sys

a, b = map(int, sys.stdin.read().split())
sys.stdout.buffer.write(f"{a + b}\n".encode("ascii"))
"#,
    )
    .unwrap();
}

pub fn configure_checker_python_problem(problem_dir: &Path) {
    std::fs::write(
        problem_dir.join("problem.yaml"),
        r#"name: checker_problem
programs:
  gen:
    info: !python
      path: ./src/gen.py
    time_limit_secs: 1.0
    memory_limit_mb: 128.0
  std:
    info: !python
      path: ./src/std.py
    time_limit_secs: 1.0
    memory_limit_mb: 128.0
  alt:
    info: !python
      path: ./src/alt.py
    time_limit_secs: 1.0
    memory_limit_mb: 128.0
  bad:
    info: !python
      path: ./src/bad.py
    time_limit_secs: 1.0
    memory_limit_mb: 128.0
  chk:
    info: !python
      path: ./src/chk.py
    time_limit_secs: 1.0
    memory_limit_mb: 128.0
solution: std
checker: chk
test:
  generator: gen
  bundles:
    sample:
      cases:
      - ["7"]
  tasks:
  - name: sample
    score: 100.0
    type: min
    bundles: [sample]
    pass: [alt]
    fail: [bad]
"#,
    )
    .unwrap();
    std::fs::write(
        problem_dir.join("src").join("gen.py"),
        r#"import sys
sys.stdout.buffer.write((sys.argv[1] + "\n").encode("ascii"))
"#,
    )
    .unwrap();
    std::fs::write(
        problem_dir.join("src").join("std.py"),
        r#"import sys
value = int(sys.stdin.read())
sys.stdout.buffer.write(f"{value}\n".encode("ascii"))
"#,
    )
    .unwrap();
    std::fs::write(
        problem_dir.join("src").join("alt.py"),
        r#"import sys
value = int(sys.stdin.read())
sys.stdout.buffer.write(f"{value:03d}\n".encode("ascii"))
"#,
    )
    .unwrap();
    std::fs::write(
        problem_dir.join("src").join("bad.py"),
        r#"import sys
value = int(sys.stdin.read())
sys.stdout.buffer.write(f"{value + 1}\n".encode("ascii"))
"#,
    )
    .unwrap();
    std::fs::write(
        problem_dir.join("src").join("chk.py"),
        r#"import sys
input_path, output_path, answer_path = sys.argv[1:4]
out = int(open(output_path, "rb").read())
ans = int(open(answer_path, "rb").read())
if out != ans:
    if len(sys.argv) >= 5:
        open(sys.argv[4], "w", encoding="utf-8").write(f"expected {ans}, found {out}\n")
    sys.stderr.write(f"expected {ans}, found {out}\n")
    raise SystemExit(1)
"#,
    )
    .unwrap();
}

pub fn add_validator_program(problem_dir: &Path, source: &str) {
    let yaml_path = problem_dir.join("problem.yaml");
    let yaml = std::fs::read_to_string(&yaml_path).unwrap();
    let yaml = yaml.replacen(
        "  brute:\n    info: !python\n      path: ./src/solve.py\n    time_limit_secs: 1.0\n    memory_limit_mb: 128.0\n",
        "  brute:\n    info: !python\n      path: ./src/solve.py\n    time_limit_secs: 1.0\n    memory_limit_mb: 128.0\n  val:\n    info: !python\n      path: ./src/val.py\n    time_limit_secs: 1.0\n    memory_limit_mb: 128.0\n",
        1,
    );
    let yaml = yaml.replacen("solution: std\n", "solution: std\nvalidator: val\n", 1);
    std::fs::write(yaml_path, yaml).unwrap();
    std::fs::write(problem_dir.join("src").join("val.py"), source).unwrap();
}

pub fn configure_unicode_python_problem(problem_dir: &Path) {
    std::fs::write(
        problem_dir.join("problem.yaml"),
        r#"name: "求和 案例"
programs:
  gen:
    info: !python
      path: ./src/生成.py
    time_limit_secs: 1.0
    memory_limit_mb: 128.0
  std:
    info: !python
      path: ./src/求解.py
    time_limit_secs: 1.0
    memory_limit_mb: 128.0
  brute:
    info: !python
      path: ./src/求解.py
    time_limit_secs: 1.0
    memory_limit_mb: 128.0
solution: std
validator_omitted_reason: "unicode path smoke test"
test:
  bundles:
    sample:
      cases:
      - generator: gen
        args: ["你好", "世界"]
  tasks:
  - name: sample
    score: 100.0
    type: min
    bundles: [sample]
    pass: [brute]
"#,
    )
    .unwrap();
    std::fs::write(
        problem_dir.join("src").join("生成.py"),
        r#"import sys

left = sys.argv[1]
right = sys.argv[2]
sys.stdout.buffer.write(f"{left} {right}\n".encode("utf-8"))
"#,
    )
    .unwrap();
    std::fs::write(
        problem_dir.join("src").join("求解.py"),
        r#"import sys

text = sys.stdin.buffer.read().decode("utf-8").strip()
sys.stdout.buffer.write(f"答案: {text}\n".encode("utf-8"))
"#,
    )
    .unwrap();
}

pub fn configure_diverse_python_problem(problem_dir: &Path) {
    std::fs::write(
        problem_dir.join("problem.yaml"),
        r#"name: diverse_problem
programs:
  gen:
    info: !python
      path: ./src/gen.py
    time_limit_secs: 1.0
    memory_limit_mb: 128.0
  std:
    info: !python
      path: ./src/solve.py
    time_limit_secs: 1.0
    memory_limit_mb: 128.0
  brute:
    info: !python
      path: ./src/solve.py
    time_limit_secs: 1.0
    memory_limit_mb: 128.0
solution: std
validator_omitted_reason: "coverage fixture"
test:
  bundles:
    sample:
      cases:
      - generator: gen
        args: ["1"]
    main:
      cases:
      - generator: gen
        args: ["20"]
      - generator: gen
        args: ["300"]
  tasks:
  - name: sample
    score: 10.0
    type: min
    bundles: [sample]
  - name: main
    score: 90.0
    type: sum
    bundles: [main]
    dependencies: [sample]
    pass: [brute]
"#,
    )
    .unwrap();
    std::fs::write(
        problem_dir.join("src").join("gen.py"),
        r#"import sys

sys.stdout.buffer.write(f"{sys.argv[1]}\n".encode("ascii"))
"#,
    )
    .unwrap();
    std::fs::write(
        problem_dir.join("src").join("solve.py"),
        r#"import sys

value = int(sys.stdin.read())
sys.stdout.buffer.write(f"{value * value}\n".encode("ascii"))
"#,
    )
    .unwrap();
}

pub fn overwrite_generator_for_range_args(problem_dir: &Path) {
    std::fs::write(
        problem_dir.join("src").join("gen.py"),
        r#"import sys

value = int(sys.argv[1])
sys.stdout.buffer.write(f"{value} 10\n".encode("ascii"))
"#,
    )
    .unwrap();
}

pub fn append_stress_plan(problem_dir: &Path) {
    let yaml_path = problem_dir.join("problem.yaml");
    let mut yaml = std::fs::read_to_string(&yaml_path).unwrap();
    yaml.push_str(
        r#"stress:
  plans:
  - name: tiny
    generator: gen
    args: ["3", "4"]
    against: [std, brute]
    cases: 2
"#,
    );
    std::fs::write(yaml_path, yaml).unwrap();
}

pub fn append_expect_task_with_range_args(problem_dir: &Path) {
    let yaml_path = problem_dir.join("problem.yaml");
    let mut yaml = std::fs::read_to_string(&yaml_path).unwrap();
    yaml = yaml.replacen(
        "  tasks:\n  - name: sample\n    score: 100.0\n    type: min\n    bundles: [sample]\n    pass: [brute]\n",
        "  tasks:\n  - name: sample\n    score: 100.0\n    type: min\n    bundles: [sample]\n    pass: [brute]\n  - name: range-proof\n    cases:\n    - generator: gen\n      args: [\"{1:2}\"]\n    pass: [brute]\n",
        1,
    );
    std::fs::write(yaml_path, yaml).unwrap();
}

pub fn append_expect_fail_stress_plan(problem_dir: &Path) {
    let yaml_path = problem_dir.join("problem.yaml");
    let mut yaml = std::fs::read_to_string(&yaml_path).unwrap();
    yaml = yaml.replacen(
        "  brute:\n    info: !python\n      path: ./src/solve.py\n    time_limit_secs: 1.0\n    memory_limit_mb: 128.0\n",
        "  brute:\n    info: !python\n      path: ./src/solve.py\n    time_limit_secs: 1.0\n    memory_limit_mb: 128.0\n  bad:\n    info: !python\n      path: ./src/bad.py\n    time_limit_secs: 1.0\n    memory_limit_mb: 128.0\n",
        1,
    );
    yaml.push_str(
        r#"stress:
  plans:
  - name: bad-is-detected
    generator: gen
    args: ["3", "4"]
    against: [std, bad]
    cases: 3
    expect: fail
"#,
    );
    std::fs::write(yaml_path, yaml).unwrap();
}

pub fn append_mixed_stress_plans(problem_dir: &Path) {
    let yaml_path = problem_dir.join("problem.yaml");
    let mut yaml = std::fs::read_to_string(&yaml_path).unwrap();
    yaml = yaml.replacen(
        "  brute:\n    info: !python\n      path: ./src/solve.py\n    time_limit_secs: 1.0\n    memory_limit_mb: 128.0\n",
        "  brute:\n    info: !python\n      path: ./src/solve.py\n    time_limit_secs: 1.0\n    memory_limit_mb: 128.0\n  bad:\n    info: !python\n      path: ./src/bad.py\n    time_limit_secs: 1.0\n    memory_limit_mb: 128.0\n",
        1,
    );
    yaml.push_str(
        r#"stress:
  plans:
  - name: tiny-pass
    generator: gen
    args: ["3", "4"]
    against: [std, brute]
    cases: 2
  - name: bad-is-detected
    generator: gen
    args: ["3", "4"]
    against: [std, bad]
    cases: 2
    expect: fail
"#,
    );
    std::fs::write(yaml_path, yaml).unwrap();
}

pub fn count_failure_reports(problem_dir: &Path) -> usize {
    let failure_dir = problem_dir.join(".cptool").join("failures");
    std::fs::read_dir(failure_dir)
        .unwrap()
        .filter(|entry| {
            entry
                .as_ref()
                .ok()
                .and_then(|entry| entry.path().extension().map(|extension| extension == "txt"))
                .unwrap_or(false)
        })
        .count()
}

pub fn release_generation_lock_after(
    problem_dir: &Path,
    delay: Duration,
) -> std::thread::JoinHandle<()> {
    let lock_dir = problem_dir.join("data").join(".cptool-gen.lock");
    std::fs::create_dir_all(&lock_dir).unwrap();
    std::thread::spawn(move || {
        std::thread::sleep(delay);
        let _ = std::fs::remove_dir_all(lock_dir);
    })
}

pub fn run_cptool<const N: usize>(args: [&str; N], trailing_path: Option<&Path>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cptool"));
    command.args(args);
    if let Some(path) = trailing_path {
        command.arg(path);
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "cptool failed\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

pub fn run_cptool_allow_failure<const N: usize>(
    args: [&str; N],
    trailing_path: Option<&Path>,
) -> Output {
    run_cptool_slice_allow_failure(&args, trailing_path)
}

pub fn run_cptool_slice_allow_failure(args: &[&str], trailing_path: Option<&Path>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cptool"));
    command.args(args);
    if let Some(path) = trailing_path {
        command.arg(path);
    }
    command.output().unwrap()
}

pub fn copy_example_tree(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let source_path = entry.path();
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();
        let target_path = dst.join(&file_name);

        if source_path.is_dir() {
            if matches!(
                name.as_ref(),
                ".cptool" | "data" | "export" | "output" | "tmp"
            ) {
                continue;
            }
            copy_example_tree(&source_path, &target_path);
        } else {
            if matches!(
                source_path
                    .extension()
                    .and_then(|extension| extension.to_str()),
                Some("exe" | "tmp")
            ) {
                continue;
            }
            std::fs::copy(&source_path, &target_path).unwrap();
        }
    }
}

pub struct TempWorkspace {
    path: PathBuf,
}

impl TempWorkspace {
    pub fn new(prefix: &str) -> Self {
        let path = std::env::temp_dir().join(format!("{prefix}-{}", temp_suffix()));
        std::fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
