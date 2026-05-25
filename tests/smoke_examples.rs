mod common;
use common::*;
use std::path::PathBuf;

#[test]
fn cli_runs_init_generate_run_batch_and_export_flow() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-full-flow");
    run_cptool(["pkg", "init", "flow_problem", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("flow_problem");
    configure_python_problem(&problem_dir);

    run_cptool(["case", "gen", "-w"], Some(&problem_dir));

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
            "case",
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
            "test",
            "batch",
            "-w",
            problem_dir.to_str().unwrap(),
            "--generator",
            "gen",
            "--pass",
            "brute",
            "--",
            "{5:7}",
            "8",
        ],
        None,
    );

    run_cptool(
        [
            "pkg",
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

#[test]
fn pkg_explain_reports_roles_data_expect_and_multiple_generators() {
    let temp = TempWorkspace::new("cptool-pkg-explain");
    let problem_dir = temp.path().join("explain_problem");
    std::fs::create_dir_all(&problem_dir).unwrap();
    std::fs::write(
        problem_dir.join("problem.yaml"),
        r#"name: explain_problem
programs:
  gen: ./src/gen.cpp
  gen_special: ./src/gen_special.cpp
  std: ./src/std.cpp
  brute: ./src/brute.cpp
  wrong_greedy: ./src/wrong_greedy.cpp
  val: ./src/val.cpp
  chk: ./src/chk.cpp
solution: std
validator: val
checker: chk
generator: gen
test:
  type: min
  bundles:
    sample:
    - [sample, 1]
    mixed:
      cases:
      - [small, 1]
      - {generator: gen_special, args: [dense, 100]}
      - {generator: ":file", args: [fixtures/input/sample1.in]}
  tasks:
  - name: official
    score: 100.0
    bundles: [sample, mixed]
    pass: [brute]
  - name: wrong-proof
    cases:
    - [small, "{1:2}"]
    - {generator: gen_special, args: [edge, "{1:3}"]}
    fail: [wrong_greedy]
"#,
    )
    .unwrap();

    let text = run_cptool(["pkg", "explain", "-w"], Some(&problem_dir));
    let stdout = String::from_utf8_lossy(&text.stdout);
    assert!(stdout.contains("roles"));
    assert!(stdout.contains("solution: std -> src/std.cpp (cpp)"));
    assert!(stdout.contains("generator: gen -> src/gen.cpp (cpp)"));
    assert!(stdout.contains("bundle mixed cases=3 generators=[:file,gen,gen_special]"));
    assert!(stdout.contains("task wrong-proof verify-only"));
    assert!(stdout.contains("fixtures/input/sample1.in used_by=[mixed[2]]"));

    let json = run_cptool(["pkg", "explain", "--json", "-w"], Some(&problem_dir));
    let value: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(value["roles"]["solution"]["name"], "std");
    assert_eq!(value["roles"]["generator"]["name"], "gen");
    assert_eq!(value["official_data"]["tasks"][0]["name"], "official");
    assert_eq!(
        value["official_data"]["bundles"]
            .as_array()
            .unwrap()
            .iter()
            .find(|bundle| bundle["name"] == "mixed")
            .unwrap()["generators"],
        serde_json::json!([":file", "gen", "gen_special"])
    );
    assert_eq!(value["expect_checks"][1]["inline_cases"], 5);
    assert_eq!(
        value["handwritten_inputs"][0]["path"],
        "fixtures/input/sample1.in"
    );
}

#[test]
fn pkg_explain_does_not_write_legacy_stress_migration() {
    let temp = TempWorkspace::new("cptool-pkg-explain-read-only");
    let problem_dir = temp.path().join("legacy_explain");
    std::fs::create_dir_all(&problem_dir).unwrap();
    let original = r#"name: legacy_explain
programs:
  gen: ./src/gen.cpp
  std: ./src/std.cpp
  brute: ./src/brute.cpp
solution: std
validator_omitted_reason: "legacy explain smoke"
generator: gen
test:
  bundles:
    sample:
    - [sample]
  tasks:
  - name: sample
    score: 100.0
    type: min
    bundles: [sample]
stress:
  plans:
  - name: old-proof
    generator: gen
    args: ["{case}"]
    against: [std, brute]
    cases: 2
    expect: fail
"#;
    std::fs::write(problem_dir.join("problem.yaml"), original).unwrap();

    let json = run_cptool(["pkg", "explain", "--json", "-w"], Some(&problem_dir));
    let value: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();

    assert_eq!(value["expect_checks"][0]["name"], "old-proof");
    assert_eq!(
        std::fs::read_to_string(problem_dir.join("problem.yaml")).unwrap(),
        original
    );
}

#[test]
fn init_scaffold_includes_working_testlib_validator() {
    let temp = TempWorkspace::new("cptool-init-testlib-validator");
    run_cptool(
        ["pkg", "init", "testlib_validator", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("testlib_validator");

    assert!(problem_dir.join("src").join("val.cpp").exists());
    assert!(problem_dir.join("src").join("testlib.h").exists());
    std::fs::write(
        problem_dir.join("src").join("gen.cpp"),
        "int main(){return 0;}\n",
    )
    .unwrap();
    std::fs::write(
        problem_dir.join("src").join("std.cpp"),
        "#include <iostream>\nint main(){std::cout << 0 << '\\n';}\n",
    )
    .unwrap();

    run_cptool(["case", "gen", "-w"], Some(&problem_dir));
    let check = run_cptool(["pkg", "check", "--json", "-w"], Some(&problem_dir));
    let value: serde_json::Value = serde_json::from_slice(&check.stdout).unwrap();

    assert_eq!(value["status"], "pass");
    assert!(
        !value["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|issue| issue["code"] == "validator_missing")
    );
}

#[test]
fn add_checker_builtin_copies_source_and_check_accepts_package() {
    let temp = TempWorkspace::new("cptool-add-checker-cli");
    run_cptool(["pkg", "init", "checker_cli", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("checker_cli");
    std::fs::write(
        problem_dir.join("src").join("gen.cpp"),
        "int main(){return 0;}\n",
    )
    .unwrap();
    std::fs::write(
        problem_dir.join("src").join("std.cpp"),
        "#include <iostream>\nint main(){std::cout << \"OK\\n\";}\n",
    )
    .unwrap();

    run_cptool(
        [
            "add",
            "checker",
            "chk",
            "-w",
            problem_dir.to_str().unwrap(),
            "--builtin",
            "wcmp",
            "--replace",
        ],
        None,
    );

    let checker_source = std::fs::read_to_string(problem_dir.join("src").join("chk.cpp")).unwrap();
    assert!(checker_source.starts_with("// Copied from testlib checkers/wcmp.cpp\n"));
    assert!(checker_source.contains("#include \"testlib.h\""));
    let problem_yaml = std::fs::read_to_string(problem_dir.join("problem.yaml")).unwrap();
    assert!(problem_yaml.contains("checker: chk\n"));
    assert!(problem_yaml.contains("chk: \"./src/chk.cpp\""));

    run_cptool(["case", "gen", "-w"], Some(&problem_dir));
    let check = run_cptool(["pkg", "check", "--json", "-w"], Some(&problem_dir));
    let value: serde_json::Value = serde_json::from_slice(&check.stdout).unwrap();
    assert_eq!(value["status"], "pass");
}

#[test]
fn example_problem_packages_generate_and_check() {
    let temp = TempWorkspace::new("cptool-example-packages");
    let example_src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("example");
    let example_dst = temp.path().join("example");
    copy_example_tree(&example_src, &example_dst);
    let testlib_src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("third_party")
        .join("testlib");
    let testlib_dst = temp.path().join("third_party").join("testlib");
    copy_example_tree(&testlib_src, &testlib_dst);

    let mut checked = Vec::new();
    for entry in std::fs::read_dir(&example_dst).unwrap() {
        let problem_dir = entry.unwrap().path();
        if !problem_dir.join("problem.yaml").is_file() {
            continue;
        }
        let work_dir = problem_dir.to_str().unwrap();
        let gen_output = run_cptool(["case", "gen", "-w", work_dir, "--summary-only"], None);
        let gen_stdout = String::from_utf8_lossy(&gen_output.stdout);
        assert!(gen_stdout.contains("gen: ok"));

        let check = run_cptool(["pkg", "check", "-w", work_dir], None);
        let check_stdout = String::from_utf8_lossy(&check.stdout);
        assert!(check_stdout.contains("status: `PASS`"));
        checked.push(
            problem_dir
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
        );
    }

    checked.sort();
    assert_eq!(checked, vec!["a_plus_b".to_string(), "sum".to_string()]);
}

#[test]
fn add_validator_registers_detected_source_and_check_accepts_package() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-add-validator-cli");
    run_cptool(
        ["pkg", "init", "validator_cli", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("validator_cli");
    configure_python_problem(&problem_dir);
    std::fs::remove_file(problem_dir.join("src").join("val.cpp")).unwrap();
    std::fs::write(
        problem_dir.join("src").join("val.py"),
        r#"import sys
data = sys.stdin.read().strip().split()
if len(data) != 2:
    raise SystemExit(1)
int(data[0])
int(data[1])
"#,
    )
    .unwrap();

    run_cptool(
        [
            "add",
            "validator",
            "val",
            "-w",
            problem_dir.to_str().unwrap(),
        ],
        None,
    );

    let problem_yaml = std::fs::read_to_string(problem_dir.join("problem.yaml")).unwrap();
    assert!(problem_yaml.contains("validator: val\n"));
    assert!(problem_yaml.contains("path: \"./src/val.py\""));

    let gen_output = run_cptool(
        [
            "case",
            "gen",
            "-w",
            problem_dir.to_str().unwrap(),
            "--summary-only",
            "--json",
        ],
        None,
    );
    let gen_value: serde_json::Value = serde_json::from_slice(&gen_output.stdout).unwrap();
    assert_eq!(gen_value["validator_configured"], true);
    assert_eq!(gen_value["validator_calls"], 1);
    let check = run_cptool(
        [
            "pkg",
            "check",
            "-w",
            problem_dir.to_str().unwrap(),
            "--json",
        ],
        None,
    );
    let value: serde_json::Value = serde_json::from_slice(&check.stdout).unwrap();
    assert_eq!(value["status"], "pass");
    assert!(
        !value["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|issue| issue["code"] == "validator_missing")
    );
}

#[test]
fn unicode_paths_and_utf8_data_flow_through_cli() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-unicode 路径");
    run_cptool(
        ["pkg", "init", "unicode_problem", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("unicode_problem");
    configure_unicode_python_problem(&problem_dir);

    run_cptool(["case", "gen", "-w"], Some(&problem_dir));

    assert_eq!(
        std::fs::read_to_string(problem_dir.join("data").join("sample-0.in")).unwrap(),
        "你好 世界\n"
    );
    assert_eq!(
        std::fs::read_to_string(problem_dir.join("data").join("sample-0.ans")).unwrap(),
        "答案: 你好 世界\n"
    );

    let stdout_path = problem_dir.join("输出 结果.out");
    run_cptool(
        [
            "case",
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
    assert_eq!(
        std::fs::read_to_string(&stdout_path).unwrap(),
        "答案: 你好 世界\n"
    );

    let check = run_cptool(["pkg", "check", "-w"], Some(&problem_dir));
    let check_stdout = String::from_utf8_lossy(&check.stdout);
    assert!(check_stdout.contains("status: `PASS`"));
    let check_json = run_cptool(["pkg", "check", "--json", "-w"], Some(&problem_dir));
    let check_json_stdout = std::str::from_utf8(&check_json.stdout).unwrap();
    assert!(check_json_stdout.contains("cptool-unicode 路径"));
    let check_json_value: serde_json::Value = serde_json::from_slice(&check_json.stdout).unwrap();
    assert_eq!(check_json_value["status"], "pass");

    run_cptool(
        [
            "pkg",
            "export",
            "-w",
            problem_dir.to_str().unwrap(),
            "--oj",
            "syzoj",
        ],
        None,
    );

    let export_dir = problem_dir.join("export").join("syzoj");
    assert_eq!(
        std::fs::read_to_string(export_dir.join("0.in")).unwrap(),
        "你好 世界\n"
    );
    assert_eq!(
        std::fs::read_to_string(export_dir.join("0.ans")).unwrap(),
        "答案: 你好 世界\n"
    );
}
#[test]
fn cli_help_describes_new_workflow_commands() {
    let version = run_cptool(["--version"], None);
    let version_stdout = String::from_utf8_lossy(&version.stdout);
    assert!(version_stdout.contains(env!("CARGO_PKG_VERSION")));
    assert!(version_stdout.contains("(commit "));

    let top = run_cptool(["--help"], None);
    let top_stdout = String::from_utf8_lossy(&top.stdout);

    for command in ["pkg", "add", "case", "test", "report"] {
        assert!(top_stdout.contains(command));
    }

    let init_help = run_cptool(["pkg", "init", "--help"], None);
    let init_help_stdout = String::from_utf8_lossy(&init_help.stdout);
    assert!(init_help_stdout.contains("Create a minimal competitive-programming problem package"));
    assert!(!init_help_stdout.contains("autocpp"));

    let gen_help = run_cptool(["case", "gen", "--help"], None);
    let gen_stdout = String::from_utf8_lossy(&gen_help.stdout);
    assert!(!gen_stdout.contains("--clean"));
    assert!(gen_stdout.contains("always rebuilds the output directory"));
    assert!(gen_stdout.contains("--summary-only"));
    assert!(gen_stdout.contains("compact generation summary"));
    assert!(gen_stdout.contains("--json"));
    assert!(gen_stdout.contains("--wait-for-generation-lock"));

    let run = run_cptool(["case", "run", "--help"], None);
    let run_stdout = String::from_utf8_lossy(&run.stdout);
    assert!(run_stdout.contains("--summary-only"));
    assert!(run_stdout.contains("Print only status"));
    assert!(run_stdout.contains("--json"));
    assert!(run_stdout.contains("--time-limit-secs"));
    assert!(run_stdout.contains("--memory-limit-mb"));
    assert!(run_stdout.contains("--wait-for-generation-lock"));

    let check = run_cptool(["pkg", "check", "--help"], None);
    let check_stdout = String::from_utf8_lossy(&check.stdout);
    assert!(check_stdout.contains("Check common package structure"));
    assert!(check_stdout.contains("--json"));
    assert!(check_stdout.contains("--wait-for-generation-lock"));

    let explain = run_cptool(["pkg", "explain", "--help"], None);
    let explain_stdout = String::from_utf8_lossy(&explain.stdout);
    assert!(explain_stdout.contains("Explain problem.yaml roles"));
    assert!(explain_stdout.contains("--json"));

    let test = run_cptool(["test", "--help"], None);
    let test_stdout = String::from_utf8_lossy(&test.stdout);
    assert!(test_stdout.contains("validator"));
    assert!(test_stdout.contains("checker"));
    let fixture = run_cptool(["fixture", "--help"], None);
    let fixture_stdout = String::from_utf8_lossy(&fixture.stdout);
    assert!(fixture_stdout.contains("add"));
    assert!(fixture_stdout.contains("check"));
    let test_validator = run_cptool(["test", "validator", "--help"], None);
    let test_validator_stdout = String::from_utf8_lossy(&test_validator.stdout);
    assert!(test_validator_stdout.contains("--input"));
    assert!(test_validator_stdout.contains("--fixture"));
    assert!(test_validator_stdout.contains("--expect"));
    assert!(test_validator_stdout.contains("--no-fix-line-endings"));
    assert!(test_validator_stdout.contains("--line-ending-hints"));
    assert!(!test_validator_stdout.contains("--stdin-text"));

    let add_checker = run_cptool(["add", "checker", "--help"], None);
    let add_checker_stdout = String::from_utf8_lossy(&add_checker.stdout);
    assert!(add_checker_stdout.contains("optionally copying a built-in"));
    assert!(add_checker_stdout.contains("--builtin"));
    let add_validator = run_cptool(["add", "validator", "--help"], None);
    let add_validator_stdout = String::from_utf8_lossy(&add_validator.stdout);
    assert!(add_validator_stdout.contains("Register a validator"));
    assert!(add_validator_stdout.contains("--replace"));
    assert!(add_validator_stdout.contains("--time-limit-secs"));

    let clean = run_cptool(["pkg", "clean", "--help"], None);
    let clean_stdout = String::from_utf8_lossy(&clean.stdout);
    assert!(clean_stdout.contains("--data"));
    assert!(clean_stdout.contains("--cache"));
    assert!(clean_stdout.contains("--json"));

    let evidence = run_cptool(["report", "evidence", "--help"], None);
    let evidence_stdout = String::from_utf8_lossy(&evidence.stdout);
    assert!(evidence_stdout.contains("Collect check, generation, and task evidence"));
    assert!(evidence_stdout.contains("--json"));
    assert!(evidence_stdout.contains("--skip-gen"));
    assert!(evidence_stdout.contains("--reuse-existing-task"));
    assert!(evidence_stdout.contains("--wait-for-generation-lock"));
    assert!(evidence_stdout.contains("--out"));

    let expect = run_cptool(["test", "expect", "--help"], None);
    let expect_stdout = String::from_utf8_lossy(&expect.stdout);
    assert!(expect_stdout.contains("--name"));
    assert!(expect_stdout.contains("Run only the named task"));
    assert!(expect_stdout.contains("--summary-only"));
    assert!(!expect_stdout.contains("--positive-only"));
    assert!(!expect_stdout.contains("--negative-only"));
    assert!(expect_stdout.contains("--json"));
    assert!(expect_stdout.contains("--wait-for-generation-lock"));

    let batch = run_cptool(["test", "batch", "--help"], None);
    let batch_stdout = String::from_utf8_lossy(&batch.stdout);
    assert!(batch_stdout.contains("--pass"));
    assert!(batch_stdout.contains("--fail"));
    assert!(batch_stdout.contains("{L:R}"));
    assert!(batch_stdout.contains("--json"));
    assert!(!batch_stdout.contains("--against"));

    for args in [
        &["test", "expect", "--positive-only"][..],
        &["test", "expect", "--negative-only"][..],
        &["test", "task", "--help"][..],
        &["test", "plan", "--help"][..],
        &["test", "stress", "--help"][..],
    ] {
        let output = run_cptool_slice_allow_failure(args, None);
        assert!(!output.status.success(), "{args:?} unexpectedly succeeded");
    }
}
#[test]
fn wait_for_generation_lock_rejects_zero_seconds() {
    for args in [
        &["case", "gen", "--wait-for-generation-lock", "0"][..],
        &["case", "run", "--wait-for-generation-lock", "0"][..],
        &["pkg", "check", "--wait-for-generation-lock", "0"][..],
        &["test", "expect", "--wait-for-generation-lock", "0"][..],
        &["report", "evidence", "--wait-for-generation-lock", "0"][..],
    ] {
        let output = run_cptool_slice_allow_failure(args, None);
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert!(!output.status.success());
        assert!(stderr.contains("value must be at least 1 second"));
    }
}

#[test]
fn legacy_top_level_commands_are_not_supported() {
    for command in [
        "init",
        "config",
        "run",
        "judge",
        "gen",
        "clean",
        "stress",
        "stress-plan",
        "check",
        "evidence",
        "export",
    ] {
        let output = run_cptool_slice_allow_failure(&[command, "--help"], None);
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert!(!output.status.success(), "{command} unexpectedly succeeded");
        assert!(stderr.contains("unrecognized subcommand"));
    }
}
