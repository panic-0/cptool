use clap::{Parser, Subcommand, ValueEnum};
use cptool::export::OnlineJudge;
use cptool::tool::DEFAULT_OUTPUT_LIMIT_BYTES;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    version = env!("CPTOOL_VERSION"),
    about = "Deterministic competitive-programming problem package tool",
    long_about = "cptool groups package lifecycle, configuration edits, case generation/runs, test workflows, and audit reports for deterministic competitive-programming problem packages."
)]
pub(super) struct Cli {
    #[command(subcommand)]
    pub(super) command: Commands,
}

#[derive(Debug, Subcommand)]
pub(super) enum Commands {
    #[command(about = "Package lifecycle, health checks, cleanup, and export")]
    Pkg {
        #[command(subcommand)]
        command: PkgCommands,
    },
    #[command(about = "Edit problem.yaml and create simple package scaffolds")]
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
    #[command(about = "Generate official cases and run configured programs")]
    Case {
        #[command(subcommand)]
        command: CaseCommands,
    },
    #[command(about = "Run validator, checker, stress, and stress-plan tests")]
    Test {
        #[command(subcommand)]
        command: TestCommands,
    },
    #[command(about = "Create, list, and check package fixtures")]
    Fixture {
        #[command(subcommand)]
        command: FixtureCommands,
    },
    #[command(about = "Collect audit and evidence reports")]
    Report {
        #[command(subcommand)]
        command: ReportCommands,
    },
}

#[derive(Debug, Subcommand)]
pub(super) enum PkgCommands {
    #[command(about = "Create a minimal competitive-programming problem package")]
    Init {
        #[arg(help = "Problem id or display name used to create <problems-dir>/<slug>")]
        id: String,
        #[arg(
            short,
            long,
            default_value = ".",
            help = "Workspace root that receives problems/, or the problems/ directory itself"
        )]
        root: PathBuf,
    },
    #[command(about = "Check common package structure, config, data, and sample issues")]
    Check {
        #[arg(short, long, default_value = ".", help = "Problem package directory")]
        work_dir: PathBuf,
        #[arg(
            long,
            value_name = "SECONDS",
            value_parser = positive_seconds,
            help = "Wait up to SECONDS for an in-progress data generation lock"
        )]
        wait_for_generation_lock: Option<u64>,
        #[arg(long, help = "Print the check report as JSON")]
        json: bool,
    },
    #[command(about = "Clean generated data and local cptool cache")]
    Clean {
        #[arg(short, long, default_value = ".", help = "Problem package directory")]
        work_dir: PathBuf,
        #[arg(long, help = "Clean generated data files from data/")]
        data: bool,
        #[arg(long, help = "Clean local cptool cache from .cptool/cache")]
        cache: bool,
        #[arg(long, help = "Print the clean report as JSON")]
        json: bool,
    },
    #[command(about = "Export the package to an online judge format")]
    Export {
        #[arg(short, long, default_value = ".", help = "Problem package directory")]
        work_dir: PathBuf,
        #[arg(long, value_enum, help = "Target online judge format")]
        oj: OnlineJudge,
    },
}

#[derive(Debug, Subcommand)]
pub(super) enum ConfigCommands {
    #[command(about = "Add programs, bundles, tasks, validators, or checkers to problem.yaml")]
    Add {
        #[command(subcommand)]
        command: AddCommands,
    },
}

#[derive(Debug, Subcommand)]
pub(super) enum CaseCommands {
    #[command(about = "Generate official .in/.ans data from problem.yaml bundles")]
    Gen {
        #[arg(short, long, default_value = ".", help = "Problem package directory")]
        work_dir: PathBuf,
        #[arg(long, help = "Generate every case in this bundle")]
        bundle: Option<String>,
        #[arg(long, help = "Generate one case selector such as large[0]")]
        case: Option<String>,
        #[arg(short, long, help = "Output directory; defaults to <work-dir>/data")]
        output_dir: Option<PathBuf>,
        #[arg(long, default_value_t = DEFAULT_OUTPUT_LIMIT_BYTES, help = "Per-stream stdout/stderr capture limit in bytes")]
        output_limit_bytes: usize,
        #[arg(
            long,
            value_name = "SECONDS",
            value_parser = positive_seconds,
            help = "Wait up to SECONDS for an in-progress data generation lock"
        )]
        wait_for_generation_lock: Option<u64>,
        #[arg(
            long,
            help = "Remove stale .in/.ans files for the selected case, bundle, or known bundles before publishing new data"
        )]
        clean: bool,
        #[arg(
            long,
            help = "Print one compact generation summary instead of each generated path"
        )]
        summary_only: bool,
        #[arg(long, help = "Print the generation report as JSON")]
        json: bool,
    },
    #[command(about = "Run a configured program or source file on package input")]
    Run {
        #[arg(help = "Program name from problem.yaml, or omit to run the configured solution")]
        program: Option<String>,
        #[arg(
            help = "Bundle case selector such as sample[0]; defaults to the first configured case"
        )]
        case: Option<String>,
        #[arg(short, long, default_value = ".", help = "Problem package directory")]
        work_dir: PathBuf,
        #[arg(
            long,
            help = "Run an explicit .cpp/.py source instead of a configured program"
        )]
        source: Option<PathBuf>,
        #[arg(long, help = "Use this literal text as stdin")]
        stdin_text: Option<String>,
        #[arg(
            long,
            help = "Read stdin from this path, relative to the package when not absolute"
        )]
        stdin_path: Option<PathBuf>,
        #[arg(
            long,
            help = "Write raw stdout bytes to this path instead of printing them"
        )]
        stdout_path: Option<PathBuf>,
        #[arg(
            long,
            help = "Write raw stderr bytes to this path instead of printing them"
        )]
        stderr_path: Option<PathBuf>,
        #[arg(long, default_value_t = DEFAULT_OUTPUT_LIMIT_BYTES, help = "Per-stream stdout/stderr capture limit in bytes")]
        output_limit_bytes: usize,
        #[arg(
            long,
            value_name = "SECONDS",
            value_parser = positive_f64,
            help = "Override this run's configured time limit in seconds"
        )]
        time_limit_secs: Option<f64>,
        #[arg(
            long,
            value_name = "MB",
            value_parser = positive_f64,
            help = "Override this run's configured memory limit in megabytes"
        )]
        memory_limit_mb: Option<f64>,
        #[arg(
            long,
            value_name = "SECONDS",
            value_parser = positive_seconds,
            help = "Wait up to SECONDS for an in-progress data generation lock"
        )]
        wait_for_generation_lock: Option<u64>,
        #[arg(
            long,
            help = "Print only status, size, line count, hash, and stderr summary"
        )]
        summary_only: bool,
        #[arg(
            long,
            help = "Print a JSON run summary; suppresses raw stdout/stderr terminal output"
        )]
        json: bool,
        #[arg(
            long,
            help = "Hide stdout in the terminal while preserving the status line and stderr"
        )]
        hide_stdout: bool,
        #[arg(last = true, help = "Extra arguments passed to the program after --")]
        args: Vec<String>,
    },
}

#[derive(Debug, Subcommand)]
pub(super) enum TestCommands {
    #[command(about = "Run validator fixtures or one explicit input file")]
    Validator {
        #[arg(short, long, default_value = ".", help = "Problem package directory")]
        work_dir: PathBuf,
        #[arg(
            long,
            help = "Validator program key; defaults to problem.yaml validator"
        )]
        validator: Option<String>,
        #[arg(long, value_name = "PATH", help = "Run one explicit input file")]
        input: Option<PathBuf>,
        #[arg(
            long,
            value_name = "pass/NAME|fail/NAME",
            help = "Run one validator fixture"
        )]
        fixture: Option<String>,
        #[arg(long, value_enum, default_value_t = JudgeExpectationArg::Pass, help = "Expected verdict")]
        expect: JudgeExpectationArg,
        #[arg(long, default_value_t = DEFAULT_OUTPUT_LIMIT_BYTES, help = "Per-stream stdout/stderr capture limit in bytes")]
        output_limit_bytes: usize,
        #[arg(
            long,
            help = "Disable automatic native line-ending normalization before running the validator"
        )]
        no_fix_line_endings: bool,
        #[arg(long, help = "Print the test result as JSON")]
        json: bool,
    },
    #[command(about = "Run checker fixtures or one explicit input/output/answer set")]
    Checker {
        #[arg(short, long, default_value = ".", help = "Problem package directory")]
        work_dir: PathBuf,
        #[arg(long, help = "Checker program key; defaults to problem.yaml checker")]
        checker: Option<String>,
        #[arg(
            long,
            value_name = "PATH",
            help = "Explicit input file passed to the checker"
        )]
        input: Option<PathBuf>,
        #[arg(
            long,
            value_name = "PATH",
            help = "Explicit participant output file passed to the checker"
        )]
        output: Option<PathBuf>,
        #[arg(
            long,
            value_name = "PATH",
            help = "Explicit jury answer file passed to the checker"
        )]
        answer: Option<PathBuf>,
        #[arg(
            long,
            value_name = "pass/NAME|fail/NAME",
            help = "Run one checker fixture"
        )]
        fixture: Option<String>,
        #[arg(long, value_enum, default_value_t = JudgeExpectationArg::Pass, help = "Expected verdict")]
        expect: JudgeExpectationArg,
        #[arg(long, default_value_t = DEFAULT_OUTPUT_LIMIT_BYTES, help = "Per-stream stdout/stderr capture limit in bytes")]
        output_limit_bytes: usize,
        #[arg(long, help = "Print the test result as JSON")]
        json: bool,
    },
    #[command(
        about = "Stress test several programs on temporary generated inputs",
        long_about = "Stress test several programs on temporary generated inputs. Generator args after -- support {case} and {case0}; {case} is 1-based and {case0} is 0-based."
    )]
    Stress {
        #[arg(short, long, default_value = ".", help = "Problem package directory")]
        work_dir: PathBuf,
        #[arg(long, help = "Generator program name from problem.yaml or source path")]
        generator: String,
        #[arg(
            long,
            required = true,
            help = "Program name or source path to compare; pass at least two"
        )]
        against: Vec<String>,
        #[arg(
            long,
            default_value_t = 100,
            help = "Number of generated cases to test"
        )]
        cases: usize,
        #[arg(long, default_value_t = DEFAULT_OUTPUT_LIMIT_BYTES, help = "Per-stream stdout/stderr capture limit in bytes")]
        output_limit_bytes: usize,
        #[arg(long, help = "Directory for failed inputs and per-program outputs")]
        failure_dir: Option<PathBuf>,
        #[arg(long, help = "Print the stress summary as JSON")]
        json: bool,
        #[arg(
            last = true,
            help = "Arguments passed to the generator after --; supports {case} and {case0}"
        )]
        args: Vec<String>,
    },
    #[command(
        name = "plan",
        about = "Run stress plans declared in problem.yaml",
        long_about = "Run stress plans declared in problem.yaml. Plan args support {case} and {case0}; {case} is 1-based and {case0} is 0-based."
    )]
    Plan {
        #[arg(short, long, default_value = ".", help = "Problem package directory")]
        work_dir: PathBuf,
        #[arg(long, help = "Run only the named stress plan; omit to run all plans")]
        name: Option<String>,
        #[arg(long, default_value_t = DEFAULT_OUTPUT_LIMIT_BYTES, help = "Per-stream stdout/stderr capture limit in bytes")]
        output_limit_bytes: usize,
        #[arg(long, help = "Directory for failed inputs and per-program outputs")]
        failure_dir: Option<PathBuf>,
        #[arg(
            long,
            value_name = "SECONDS",
            value_parser = positive_seconds,
            help = "Wait up to SECONDS for an in-progress data generation lock"
        )]
        wait_for_generation_lock: Option<u64>,
        #[arg(
            long,
            help = "Print one compact summary line per plan instead of per-case progress"
        )]
        summary_only: bool,
        #[arg(
            long,
            conflicts_with = "negative_only",
            help = "Run only expect: pass plans"
        )]
        positive_only: bool,
        #[arg(
            long,
            conflicts_with = "positive_only",
            help = "Run only expect: fail plans"
        )]
        negative_only: bool,
        #[arg(long, help = "Print stress plan summaries as JSON")]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
pub(super) enum FixtureCommands {
    #[command(about = "Add a package fixture")]
    Add {
        #[command(subcommand)]
        command: FixtureAddCommands,
    },
    #[command(about = "List package fixtures")]
    List {
        #[arg(short, long, default_value = ".", help = "Problem package directory")]
        work_dir: PathBuf,
        #[arg(long, help = "Print fixture list as JSON")]
        json: bool,
    },
    #[command(about = "Check fixture structure and input usage")]
    Check {
        #[arg(short, long, default_value = ".", help = "Problem package directory")]
        work_dir: PathBuf,
        #[arg(long, help = "Print fixture check report as JSON")]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
pub(super) enum FixtureAddCommands {
    #[command(about = "Add a hand-written input fixture for `:file` test cases")]
    Input {
        #[arg(help = "Fixture name without path or extension")]
        name: String,
        #[arg(short, long, default_value = ".", help = "Problem package directory")]
        work_dir: PathBuf,
        #[arg(long, value_name = "PATH", help = "Copy input bytes from this path")]
        from: Option<PathBuf>,
        #[arg(long, help = "Replace an existing fixture")]
        replace: bool,
    },
    #[command(about = "Add a validator pass/fail fixture")]
    Validator {
        #[command(subcommand)]
        command: FixtureValidatorCommands,
    },
    #[command(about = "Add a checker pass/fail fixture")]
    Checker {
        #[command(subcommand)]
        command: FixtureCheckerCommands,
    },
}

#[derive(Debug, Subcommand)]
pub(super) enum FixtureValidatorCommands {
    #[command(about = "Add an input that the validator must accept")]
    Pass {
        #[arg(help = "Fixture name without path or extension")]
        name: String,
        #[arg(short, long, default_value = ".", help = "Problem package directory")]
        work_dir: PathBuf,
        #[arg(long, value_name = "PATH", help = "Copy input bytes from this path")]
        from: Option<PathBuf>,
        #[arg(long, help = "Replace an existing fixture")]
        replace: bool,
    },
    #[command(about = "Add an input that the validator must reject")]
    Fail {
        #[arg(help = "Fixture name without path or extension")]
        name: String,
        #[arg(short, long, default_value = ".", help = "Problem package directory")]
        work_dir: PathBuf,
        #[arg(long, value_name = "PATH", help = "Copy input bytes from this path")]
        from: Option<PathBuf>,
        #[arg(long, help = "Replace an existing fixture")]
        replace: bool,
    },
}

#[derive(Debug, Subcommand)]
pub(super) enum FixtureCheckerCommands {
    #[command(about = "Add files that the checker must accept")]
    Pass {
        #[arg(help = "Fixture name without path or extension")]
        name: String,
        #[arg(short, long, default_value = ".", help = "Problem package directory")]
        work_dir: PathBuf,
        #[arg(
            long,
            value_name = "PATH",
            help = "Copy checker input bytes from this path"
        )]
        input: Option<PathBuf>,
        #[arg(
            long,
            value_name = "PATH",
            help = "Copy participant output bytes from this path"
        )]
        output: Option<PathBuf>,
        #[arg(
            long,
            value_name = "PATH",
            help = "Copy jury answer bytes from this path"
        )]
        answer: Option<PathBuf>,
        #[arg(long, help = "Replace an existing fixture")]
        replace: bool,
    },
    #[command(about = "Add files that the checker must reject")]
    Fail {
        #[arg(help = "Fixture name without path or extension")]
        name: String,
        #[arg(short, long, default_value = ".", help = "Problem package directory")]
        work_dir: PathBuf,
        #[arg(
            long,
            value_name = "PATH",
            help = "Copy checker input bytes from this path"
        )]
        input: Option<PathBuf>,
        #[arg(
            long,
            value_name = "PATH",
            help = "Copy participant output bytes from this path"
        )]
        output: Option<PathBuf>,
        #[arg(
            long,
            value_name = "PATH",
            help = "Copy jury answer bytes from this path"
        )]
        answer: Option<PathBuf>,
        #[arg(long, help = "Replace an existing fixture")]
        replace: bool,
    },
}

#[derive(Debug, Subcommand)]
pub(super) enum ReportCommands {
    #[command(about = "Collect check, generation, and stress-plan evidence")]
    Evidence {
        #[arg(short, long, default_value = ".", help = "Problem package directory")]
        work_dir: PathBuf,
        #[arg(long, default_value_t = DEFAULT_OUTPUT_LIMIT_BYTES, help = "Per-stream stdout/stderr capture limit in bytes")]
        output_limit_bytes: usize,
        #[arg(long, help = "Skip official data generation evidence")]
        skip_gen: bool,
        #[arg(long, help = "Skip stress-plan evidence")]
        skip_stress_plan: bool,
        #[arg(
            long,
            value_name = "PATH",
            conflicts_with = "skip_stress_plan",
            help = "Reuse JSON from `test plan --summary-only --json` instead of rerunning stress plans"
        )]
        reuse_existing_stress_plan: Option<PathBuf>,
        #[arg(
            long,
            value_name = "SECONDS",
            value_parser = positive_seconds,
            help = "Wait up to SECONDS for an in-progress data generation lock"
        )]
        wait_for_generation_lock: Option<u64>,
        #[arg(long, help = "Print the evidence report as JSON")]
        json: bool,
        #[arg(
            long,
            conflicts_with = "json",
            help = "Print a quality_report.md-ready Markdown evidence section"
        )]
        markdown: bool,
    },
}

#[derive(Debug, Subcommand)]
pub(super) enum AddCommands {
    #[command(about = "Register a program, auto-creating src/<name>.cpp when needed")]
    Program {
        #[arg(help = "Program key to add under problem.yaml programs")]
        name: String,
        #[arg(short, long, default_value = ".", help = "Problem package directory")]
        work_dir: PathBuf,
        #[arg(
            long,
            value_enum,
            help = "Program kind; inferred from path when omitted"
        )]
        kind: Option<AddProgramKindArg>,
        #[arg(long, help = "Program path; defaults to src/<name>.* auto-detection")]
        path: Option<PathBuf>,
        #[arg(
            long,
            value_name = "SECONDS",
            value_parser = positive_f64,
            help = "Configured program time limit in seconds"
        )]
        time_limit_secs: Option<f64>,
        #[arg(
            long,
            value_name = "MB",
            value_parser = positive_f64,
            help = "Configured program memory limit in megabytes"
        )]
        memory_limit_mb: Option<f64>,
        #[arg(
            long,
            value_name = "ARG",
            help = "C++ compile arg; replaces defaults when present"
        )]
        compile_arg: Vec<String>,
        #[arg(long, help = "Replace an existing program entry")]
        replace: bool,
    },
    #[command(about = "Register a test bundle")]
    Bundle {
        #[arg(help = "Bundle name to add under test.bundles")]
        name: String,
        #[arg(short, long, default_value = ".", help = "Problem package directory")]
        work_dir: PathBuf,
        #[arg(long, help = "Generator program for every added case; defaults to gen")]
        generator: Option<String>,
        #[arg(
            long = "case",
            required = true,
            help = "Comma-separated generator args; use an empty string for []"
        )]
        cases: Vec<String>,
        #[arg(long, help = "Replace an existing bundle")]
        replace: bool,
    },
    #[command(about = "Register a test task")]
    Task {
        #[arg(help = "Task name to add under test.tasks")]
        name: String,
        #[arg(short, long, default_value = ".", help = "Problem package directory")]
        work_dir: PathBuf,
        #[arg(long, help = "Task score")]
        score: f64,
        #[arg(long = "type", value_enum, default_value_t = AddTaskTypeArg::Min, help = "Task scoring aggregation type")]
        task_type: AddTaskTypeArg,
        #[arg(
            long = "bundle",
            required = true,
            help = "Bundle included in this task"
        )]
        bundles: Vec<String>,
        #[arg(long = "depends", help = "Task dependency")]
        dependencies: Vec<String>,
        #[arg(long, help = "Replace an existing task")]
        replace: bool,
    },
    #[command(about = "Register a validator program")]
    Validator {
        #[arg(default_value = "val", help = "Validator program key; defaults to val")]
        name: String,
        #[arg(short, long, default_value = ".", help = "Problem package directory")]
        work_dir: PathBuf,
        #[arg(
            long,
            value_name = "SECONDS",
            value_parser = positive_f64,
            help = "Configured validator time limit in seconds"
        )]
        time_limit_secs: Option<f64>,
        #[arg(
            long,
            value_name = "MB",
            value_parser = positive_f64,
            help = "Configured validator memory limit in megabytes"
        )]
        memory_limit_mb: Option<f64>,
        #[arg(
            long,
            value_name = "ARG",
            help = "C++ compile arg; replaces defaults when present"
        )]
        compile_arg: Vec<String>,
        #[arg(long, help = "Replace existing validator config or program")]
        replace: bool,
    },
    #[command(about = "Register a checker program, optionally copying a built-in testlib checker")]
    Checker {
        #[arg(default_value = "chk", help = "Checker program key; defaults to chk")]
        name: String,
        #[arg(short, long, default_value = ".", help = "Problem package directory")]
        work_dir: PathBuf,
        #[arg(long, help = "Built-in testlib checker id to copy, e.g. wcmp")]
        builtin: Option<String>,
        #[arg(
            long,
            value_name = "SECONDS",
            value_parser = positive_f64,
            help = "Configured checker time limit in seconds"
        )]
        time_limit_secs: Option<f64>,
        #[arg(
            long,
            value_name = "MB",
            value_parser = positive_f64,
            help = "Configured checker memory limit in megabytes"
        )]
        memory_limit_mb: Option<f64>,
        #[arg(
            long,
            value_name = "ARG",
            help = "C++ compile arg; replaces defaults when present"
        )]
        compile_arg: Vec<String>,
        #[arg(long, help = "Replace existing checker config or source")]
        replace: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(super) enum AddProgramKindArg {
    Cpp,
    Python,
    Command,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(super) enum AddTaskTypeArg {
    Min,
    Sum,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(super) enum JudgeExpectationArg {
    Pass,
    Fail,
}

pub(super) fn positive_seconds(value: &str) -> Result<u64, String> {
    let seconds = value
        .parse::<u64>()
        .map_err(|_| format!("`{value}` is not a positive integer number of seconds"))?;
    if seconds == 0 {
        return Err("value must be at least 1 second".to_string());
    }
    Ok(seconds)
}

pub(super) fn positive_f64(value: &str) -> Result<f64, String> {
    let number = value
        .parse::<f64>()
        .map_err(|_| format!("`{value}` is not a positive finite number"))?;
    if !number.is_finite() || number <= 0.0 {
        return Err("value must be a positive finite number".to_string());
    }
    Ok(number)
}
