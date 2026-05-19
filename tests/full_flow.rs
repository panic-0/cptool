use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[test]
fn cli_runs_init_generate_run_stress_and_export_flow() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-full-flow");
    run_cptool(["init", "flow_problem", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("problems").join("flow_problem");
    configure_python_problem(&problem_dir);

    run_cptool(["gen", "-w"], Some(&problem_dir));

    assert_eq!(
        std::fs::read_to_string(problem_dir.join("data").join("sample-0.in")).unwrap(),
        "3 4\n"
    );
    assert_eq!(
        std::fs::read_to_string(problem_dir.join("data").join("sample-0.ans")).unwrap(),
        "7\n"
    );

    let stdout_path = problem_dir.join("actual.out");
    run_cptool(
        [
            "run",
            "std",
            "sample[0]",
            "-w",
            problem_dir.to_str().unwrap(),
            "--stdout-path",
            stdout_path.to_str().unwrap(),
        ],
        None,
    );
    assert_eq!(std::fs::read_to_string(&stdout_path).unwrap(), "7\n");

    run_cptool(
        [
            "stress",
            "-w",
            problem_dir.to_str().unwrap(),
            "--generator",
            "gen",
            "--against",
            "std",
            "--against",
            "brute",
            "--cases",
            "3",
            "--",
            "5",
            "8",
        ],
        None,
    );

    run_cptool(
        [
            "export",
            "-w",
            problem_dir.to_str().unwrap(),
            "--oj",
            "syzoj",
        ],
        None,
    );

    let export_dir = problem_dir.join("export").join("syzoj");
    assert!(export_dir.join("data.yml").exists());
    assert_eq!(
        std::fs::read_to_string(export_dir.join("0.in")).unwrap(),
        "3 4\n"
    );
    assert_eq!(
        std::fs::read_to_string(export_dir.join("0.ans")).unwrap(),
        "7\n"
    );
}

fn configure_python_problem(problem_dir: &Path) {
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
  bundles:
    sample:
      cases:
      - generator: gen
        args: ["3", "4"]
  tasks:
  - name: sample
    score: 100.0
    type: min
    bundles: [sample]
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

fn run_cptool<const N: usize>(args: [&str; N], trailing_path: Option<&Path>) -> Output {
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

fn python_available() -> bool {
    let python = std::env::var("PYTHON").unwrap_or_else(|_| "python".to_string());
    Command::new(python)
        .arg("--version")
        .status()
        .is_ok_and(|status| status.success())
}

struct TempWorkspace {
    path: PathBuf,
}

impl TempWorkspace {
    fn new(prefix: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "{}-{}",
            prefix,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
