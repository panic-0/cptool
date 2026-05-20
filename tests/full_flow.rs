use cptool::test_support::{python_available, temp_suffix};
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

#[test]
fn unicode_paths_and_utf8_data_flow_through_cli() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-unicode 路径");
    run_cptool(["init", "unicode_problem", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("problems").join("unicode_problem");
    configure_unicode_python_problem(&problem_dir);

    run_cptool(["gen", "-w"], Some(&problem_dir));

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

    let check = run_cptool(["check", "-w"], Some(&problem_dir));
    let check_stdout = String::from_utf8_lossy(&check.stdout);
    assert!(check_stdout.contains("status: `PASS`"));

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

    assert!(top_stdout.contains("check"));
    assert!(top_stdout.contains("stress-plan"));

    let gen_help = run_cptool(["gen", "--help"], None);
    let gen_stdout = String::from_utf8_lossy(&gen_help.stdout);
    assert!(gen_stdout.contains("--clean"));
    assert!(gen_stdout.contains("Remove stale .in/.ans files"));
    assert!(gen_stdout.contains("--summary-only"));
    assert!(gen_stdout.contains("compact generation summary"));

    let run = run_cptool(["run", "--help"], None);
    let run_stdout = String::from_utf8_lossy(&run.stdout);
    assert!(run_stdout.contains("--summary-only"));
    assert!(run_stdout.contains("Print only status"));

    let check = run_cptool(["check", "--help"], None);
    let check_stdout = String::from_utf8_lossy(&check.stdout);
    assert!(check_stdout.contains("Check common package structure"));

    let stress_plan = run_cptool(["stress-plan", "--help"], None);
    let stress_plan_stdout = String::from_utf8_lossy(&stress_plan.stdout);
    assert!(stress_plan_stdout.contains("--name"));
    assert!(stress_plan_stdout.contains("Run only the named stress plan"));
    assert!(stress_plan_stdout.contains("--summary-only"));

    let stress = run_cptool(["stress", "--help"], None);
    let stress_stdout = String::from_utf8_lossy(&stress.stdout);
    assert!(stress_stdout.contains("{seed}"));
    assert!(stress_stdout.contains("{case}"));
    assert!(stress_stdout.contains("{case0}"));
}

#[test]
fn run_summary_only_and_hide_stdout_do_not_print_full_stdout() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-run-summary");
    run_cptool(["init", "summary_problem", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("problems").join("summary_problem");
    configure_python_problem(&problem_dir);
    run_cptool(["gen", "-w"], Some(&problem_dir));

    let summary = run_cptool(
        [
            "run",
            "std",
            "sample[0]",
            "-w",
            problem_dir.to_str().unwrap(),
            "--summary-only",
        ],
        None,
    );
    let summary_stdout = String::from_utf8_lossy(&summary.stdout);

    assert!(summary_stdout.contains("std: ok exit=0"));
    assert!(summary_stdout.contains("stdout_bytes=2"));
    assert!(summary_stdout.contains("stdout_lines=1"));
    assert!(summary_stdout.contains("stdout_sha256="));
    assert!(summary_stdout.contains("stderr_bytes=0"));
    assert!(!summary_stdout.contains("\n7\n"));

    let hidden = run_cptool(
        [
            "run",
            "std",
            "sample[0]",
            "-w",
            problem_dir.to_str().unwrap(),
            "--hide-stdout",
        ],
        None,
    );
    let hidden_stdout = String::from_utf8_lossy(&hidden.stdout);

    assert!(hidden_stdout.contains("std: ok exit=0"));
    assert!(!hidden_stdout.contains("\n7\n"));
}

#[test]
fn gen_warns_on_empty_answer_for_non_empty_input_unless_allowed() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-empty-answer");
    run_cptool(["init", "empty_answer", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("problems").join("empty_answer");
    configure_python_problem(&problem_dir);
    std::fs::write(
        problem_dir.join("src").join("solve.py"),
        "import sys\nsys.stdin.buffer.read()\n",
    )
    .unwrap();

    let result = run_cptool(["gen", "-w"], Some(&problem_dir));
    let stderr = String::from_utf8_lossy(&result.stderr);

    assert!(stderr.contains("warning: empty_answer"));
    assert!(stderr.contains("case=sample[0]"));
    assert!(stderr.contains("solution=std"));
    assert!(stderr.contains("stdout_bytes=0"));
    assert!(stderr.contains("stderr_bytes=0"));

    let summary = run_cptool(
        ["gen", "-w", problem_dir.to_str().unwrap(), "--summary-only"],
        None,
    );
    let summary_stdout = String::from_utf8_lossy(&summary.stdout);
    let summary_stderr = String::from_utf8_lossy(&summary.stderr);

    assert!(summary_stdout.contains("gen: ok cases=1 bundles=sample elapsed="));
    assert!(summary_stdout.contains("in_bytes=4"));
    assert!(summary_stdout.contains("ans_bytes=0"));
    assert!(summary_stdout.contains("warnings=empty_answer:1"));
    assert!(!summary_stdout.contains("generated "));
    assert!(!summary_stderr.contains("warning: empty_answer"));

    let yaml_path = problem_dir.join("problem.yaml");
    let yaml = std::fs::read_to_string(&yaml_path).unwrap();
    std::fs::write(
        &yaml_path,
        yaml.replacen(
            "programs:\n",
            "output:\n  allow_empty: true\nprograms:\n",
            1,
        ),
    )
    .unwrap();

    let allowed = run_cptool(["gen", "-w"], Some(&problem_dir));
    let allowed_stderr = String::from_utf8_lossy(&allowed.stderr);

    assert!(!allowed_stderr.contains("warning: empty_answer"));
}

#[test]
fn gen_summary_only_prints_compact_success_totals() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-gen-summary");
    run_cptool(["init", "gen_summary", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("problems").join("gen_summary");
    configure_python_problem(&problem_dir);

    let output = run_cptool(
        ["gen", "-w", problem_dir.to_str().unwrap(), "--summary-only"],
        None,
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("gen: ok cases=1 bundles=sample elapsed="));
    assert!(stdout.contains("in_bytes=4"));
    assert!(stdout.contains("ans_bytes=2"));
    assert!(stdout.contains("warnings=0"));
    assert!(!stdout.contains("generated "));
}

#[test]
fn gen_and_export_cover_multiple_bundles_cases_and_tasks() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-diverse-data");
    run_cptool(["init", "diverse_problem", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("problems").join("diverse_problem");
    configure_diverse_python_problem(&problem_dir);

    let summary = run_cptool(
        ["gen", "-w", problem_dir.to_str().unwrap(), "--summary-only"],
        None,
    );
    let summary_stdout = String::from_utf8_lossy(&summary.stdout);

    assert!(summary_stdout.contains("gen: ok cases=3 bundles=main,sample elapsed="));
    assert!(summary_stdout.contains("in_bytes=9"));
    assert!(summary_stdout.contains("ans_bytes=12"));
    assert!(summary_stdout.contains("warnings=0"));

    let data_dir = problem_dir.join("data");
    assert_eq!(
        std::fs::read_to_string(data_dir.join("sample-0.in")).unwrap(),
        "1\n"
    );
    assert_eq!(
        std::fs::read_to_string(data_dir.join("sample-0.ans")).unwrap(),
        "1\n"
    );
    assert_eq!(
        std::fs::read_to_string(data_dir.join("main-0.in")).unwrap(),
        "20\n"
    );
    assert_eq!(
        std::fs::read_to_string(data_dir.join("main-0.ans")).unwrap(),
        "400\n"
    );
    assert_eq!(
        std::fs::read_to_string(data_dir.join("main-1.in")).unwrap(),
        "300\n"
    );
    assert_eq!(
        std::fs::read_to_string(data_dir.join("main-1.ans")).unwrap(),
        "90000\n"
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
        "1\n"
    );
    assert_eq!(
        std::fs::read_to_string(export_dir.join("0.ans")).unwrap(),
        "1\n"
    );
    assert_eq!(
        std::fs::read_to_string(export_dir.join("1.in")).unwrap(),
        "20\n"
    );
    assert_eq!(
        std::fs::read_to_string(export_dir.join("1.ans")).unwrap(),
        "400\n"
    );
    assert_eq!(
        std::fs::read_to_string(export_dir.join("2.in")).unwrap(),
        "300\n"
    );
    assert_eq!(
        std::fs::read_to_string(export_dir.join("2.ans")).unwrap(),
        "90000\n"
    );

    let data_yml = std::fs::read_to_string(export_dir.join("data.yml")).unwrap();
    assert!(data_yml.contains("subtasks:"));
    assert!(data_yml.contains("dependencies:"));
}

#[test]
fn gen_warns_when_generator_stdout_is_empty() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-empty-generator");
    run_cptool(["init", "empty_generator", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("problems").join("empty_generator");
    configure_python_problem(&problem_dir);
    std::fs::write(
        problem_dir.join("src").join("gen.py"),
        "import sys\nsys.stderr.write('no input produced')\n",
    )
    .unwrap();
    std::fs::write(
        problem_dir.join("src").join("solve.py"),
        "import sys\nsys.stdin.buffer.read()\n",
    )
    .unwrap();

    let result = run_cptool(["gen", "-w"], Some(&problem_dir));
    let stderr = String::from_utf8_lossy(&result.stderr);

    assert!(stderr.contains("warning: generator_output_suspicious"));
    assert!(stderr.contains("case=sample[0]"));
    assert!(stderr.contains("generator=gen"));
    assert!(stderr.contains("stdout_bytes=0"));
    assert!(stderr.contains("stderr_bytes="));
}

#[test]
fn gen_clean_removes_only_selected_bundle_and_preserves_on_failure() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-gen-clean");
    run_cptool(["init", "clean_problem", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("problems").join("clean_problem");
    configure_python_problem(&problem_dir);

    run_cptool(["gen", "-w"], Some(&problem_dir));
    let data_dir = problem_dir.join("data");
    std::fs::write(data_dir.join("sample-99.in"), "stale").unwrap();
    std::fs::write(data_dir.join("sample-99.ans"), "stale").unwrap();
    std::fs::write(data_dir.join("sampleish-0.in"), "keep").unwrap();

    run_cptool(
        [
            "gen",
            "-w",
            problem_dir.to_str().unwrap(),
            "--bundle",
            "sample",
            "--clean",
        ],
        None,
    );

    assert!(!data_dir.join("sample-99.in").exists());
    assert!(!data_dir.join("sample-99.ans").exists());
    assert_eq!(
        std::fs::read_to_string(data_dir.join("sampleish-0.in")).unwrap(),
        "keep"
    );

    std::fs::write(
        problem_dir.join("src").join("gen.py"),
        "import sys\nsys.exit(1)\n",
    )
    .unwrap();
    let failed = run_cptool_allow_failure(
        [
            "gen",
            "-w",
            problem_dir.to_str().unwrap(),
            "--case",
            "sample[0]",
        ],
        None,
    );

    assert!(!failed.status.success());
    assert_eq!(
        std::fs::read_to_string(data_dir.join("sample-0.in")).unwrap(),
        "3 4\n"
    );
    assert_eq!(
        std::fs::read_to_string(data_dir.join("sample-0.ans")).unwrap(),
        "7\n"
    );
}

#[test]
fn check_command_reports_valid_and_invalid_packages() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-check-command");
    run_cptool(["init", "check_problem", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("problems").join("check_problem");
    configure_python_problem(&problem_dir);

    let ok = run_cptool(["check", "-w"], Some(&problem_dir));
    let ok_stdout = String::from_utf8_lossy(&ok.stdout);
    assert!(ok_stdout.contains("status: `PASS`"));

    std::fs::remove_file(problem_dir.join("src").join("std.cpp")).unwrap();
    let failed = run_cptool_allow_failure(["check", "-w"], Some(&problem_dir));
    let failed_stdout = String::from_utf8_lossy(&failed.stdout);

    assert!(!failed.status.success());
    assert!(failed_stdout.contains("status: `FAIL`"));
    assert!(failed_stdout.contains("required_file_missing"));
}

#[test]
fn stress_plan_runs_named_plan_without_seed_config() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-stress-plan");
    run_cptool(["init", "stress_plan_problem", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("problems").join("stress_plan_problem");
    configure_python_problem(&problem_dir);
    append_stress_plan(&problem_dir);

    let output = run_cptool(
        [
            "stress-plan",
            "-w",
            problem_dir.to_str().unwrap(),
            "--name",
            "tiny",
        ],
        None,
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("plan `tiny` case 1 ok"));
    assert!(stdout.contains("plan `tiny` case 2 ok"));
    assert!(stdout.contains("stress plan `tiny` passed: 2 cases"));
}

#[test]
fn stress_plan_summary_only_suppresses_case_progress() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-stress-plan-summary");
    run_cptool(["init", "stress_plan_summary", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("problems").join("stress_plan_summary");
    configure_python_problem(&problem_dir);
    append_stress_plan(&problem_dir);

    let output = run_cptool(
        [
            "stress-plan",
            "-w",
            problem_dir.to_str().unwrap(),
            "--name",
            "tiny",
            "--summary-only",
        ],
        None,
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("tiny: ok cases=2 unique_input_hashes=1 against=std,brute elapsed="));
    assert!(stdout.contains("unique_input_hashes=1"));
    assert!(stdout.contains("empty_stdout_cases=0"));
    assert!(stdout.contains("all_empty_stdout_cases=0"));
    assert!(stdout.contains("warnings=repeated_input:1"));
    assert!(!stdout.contains("plan `tiny` case 1 ok"));
    assert!(!stdout.contains("stress plan `tiny` passed"));
    assert!(output.stderr.is_empty());
}

#[test]
fn stress_warns_when_all_against_stdout_is_empty_on_non_empty_input() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-stress-empty-output");
    run_cptool(["init", "stress_empty_output", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("problems").join("stress_empty_output");
    configure_python_problem(&problem_dir);
    std::fs::write(
        problem_dir.join("src").join("solve.py"),
        "import sys\nsys.stdin.buffer.read()\n",
    )
    .unwrap();

    let output = run_cptool(
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
            "2",
        ],
        None,
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stdout.contains("stress passed: 2 cases"));
    assert!(stderr.contains("warning: all_empty_output case=1 against=std,brute input_bytes=4"));
    assert!(stderr.contains("warning: all_empty_output case=2 against=std,brute input_bytes=4"));
    assert!(stderr.contains(
        "warning: repeated_input cases=2 unique_input_hashes=1 hint=generator_args_produced_identical_inputs"
    ));
}

#[test]
fn stress_reports_single_unique_input_hash_for_fixed_args() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-stress-fixed-args");
    run_cptool(["init", "stress_fixed_args", "--root"], Some(temp.path()));
    let problem_dir = temp.path().join("problems").join("stress_fixed_args");
    configure_python_problem(&problem_dir);

    let output = run_cptool(
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
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stdout.contains("stress passed: 3 cases"));
    assert!(stdout.contains("unique_input_hashes=1"));
    assert!(stderr.contains(
        "warning: repeated_input cases=3 unique_input_hashes=1 hint=generator_args_produced_identical_inputs"
    ));
}

#[test]
fn stress_expands_case_placeholder_and_reports_unique_inputs() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-stress-case-placeholder");
    run_cptool(
        ["init", "stress_case_placeholder", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("problems").join("stress_case_placeholder");
    configure_python_problem(&problem_dir);

    let output = run_cptool(
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
            "{case}",
            "10",
        ],
        None,
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("case 1 ok"));
    assert!(stdout.contains("case 2 ok"));
    assert!(stdout.contains("case 3 ok"));
    assert!(stdout.contains("unique_input_hashes=3"));
}

#[test]
fn stress_plan_summary_only_reports_empty_stdout_warning_count() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-stress-plan-empty-summary");
    run_cptool(
        ["init", "stress_plan_empty_summary", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp
        .path()
        .join("problems")
        .join("stress_plan_empty_summary");
    configure_python_problem(&problem_dir);
    append_stress_plan(&problem_dir);
    std::fs::write(
        problem_dir.join("src").join("solve.py"),
        "import sys\nsys.stdin.buffer.read()\n",
    )
    .unwrap();

    let output = run_cptool(
        [
            "stress-plan",
            "-w",
            problem_dir.to_str().unwrap(),
            "--name",
            "tiny",
            "--summary-only",
        ],
        None,
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stdout.contains("tiny: ok cases=2 unique_input_hashes=1 against=std,brute elapsed="));
    assert!(stdout.contains("unique_input_hashes=1"));
    assert!(stdout.contains("empty_stdout_cases=2"));
    assert!(stdout.contains("all_empty_stdout_cases=2"));
    assert!(stdout.contains("warnings=all_empty_output:2,repeated_input:1"));
    assert!(!stderr.contains("warning: all_empty_output"));
    assert!(!stderr.contains("warning: repeated_input"));
}

#[test]
fn stress_plan_expands_seed_and_case_placeholders() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-stress-plan-placeholders");
    run_cptool(
        ["init", "stress_plan_placeholders", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp
        .path()
        .join("problems")
        .join("stress_plan_placeholders");
    configure_python_problem(&problem_dir);
    overwrite_generator_for_stress_plan_placeholders(&problem_dir);
    append_stress_plan_with_seed_placeholders(&problem_dir);

    let output = run_cptool(
        [
            "stress-plan",
            "-w",
            problem_dir.to_str().unwrap(),
            "--name",
            "seeded",
        ],
        None,
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("plan `seeded` case 1 ok"));
    assert!(stdout.contains("plan `seeded` case 2 ok"));
    assert!(stdout.contains("stress plan `seeded` passed: 2 cases"));
}

#[test]
fn stress_plan_expect_fail_treats_wrong_answer_as_success() {
    if !python_available() {
        return;
    }

    let temp = TempWorkspace::new("cptool-stress-plan-expect-fail");
    run_cptool(
        ["init", "stress_plan_expect_fail", "--root"],
        Some(temp.path()),
    );
    let problem_dir = temp.path().join("problems").join("stress_plan_expect_fail");
    configure_python_problem(&problem_dir);
    std::fs::write(
        problem_dir.join("src").join("bad.py"),
        r#"import sys

a, b = map(int, sys.stdin.read().split())
sys.stdout.buffer.write(f"{a + b + 1}\n".encode("ascii"))
"#,
    )
    .unwrap();
    append_expect_fail_stress_plan(&problem_dir);

    let output = run_cptool(
        [
            "stress-plan",
            "-w",
            problem_dir.to_str().unwrap(),
            "--name",
            "bad-is-detected",
            "--summary-only",
        ],
        None,
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("bad-is-detected: expected_fail observed=true case=1"));
    assert!(stdout.contains("reason=wrong_answer: output mismatch between `std` and `bad`"));
    assert!(
        problem_dir
            .join("tests")
            .join("failures")
            .join("stress-bad-is-detected-001.txt")
            .exists()
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

fn configure_unicode_python_problem(problem_dir: &Path) {
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

fn configure_diverse_python_problem(problem_dir: &Path) {
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

fn overwrite_generator_for_stress_plan_placeholders(problem_dir: &Path) {
    std::fs::write(
        problem_dir.join("src").join("gen.py"),
        r#"import sys

seed = int(sys.argv[1])
case = int(sys.argv[2])
case0 = int(sys.argv[3])
if case != case0 + 1:
    raise SystemExit(7)
sys.stdout.buffer.write(f"{seed} {case}\n".encode("ascii"))
"#,
    )
    .unwrap();
}

fn append_stress_plan(problem_dir: &Path) {
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

fn append_stress_plan_with_seed_placeholders(problem_dir: &Path) {
    let yaml_path = problem_dir.join("problem.yaml");
    let mut yaml = std::fs::read_to_string(&yaml_path).unwrap();
    yaml.push_str(
        r#"stress:
  plans:
  - name: seeded
    generator: gen
    args: ["{seed}", "{case}", "{case0}"]
    against: [std, brute]
    cases: 2
    seed_base: 20260519
"#,
    );
    std::fs::write(yaml_path, yaml).unwrap();
}

fn append_expect_fail_stress_plan(problem_dir: &Path) {
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

fn run_cptool_allow_failure<const N: usize>(
    args: [&str; N],
    trailing_path: Option<&Path>,
) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cptool"));
    command.args(args);
    if let Some(path) = trailing_path {
        command.arg(path);
    }
    command.output().unwrap()
}

struct TempWorkspace {
    path: PathBuf,
}

impl TempWorkspace {
    fn new(prefix: &str) -> Self {
        let path = std::env::temp_dir().join(format!("{prefix}-{}", temp_suffix()));
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
