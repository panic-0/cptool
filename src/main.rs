use clap::{Parser, Subcommand};
use cptool::export::{Exporter, OnlineJudge, syzoj};
use cptool::tool::{self, DEFAULT_OUTPUT_LIMIT_BYTES, RunOptions};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Debug, Parser)]
#[command(
    version = env!("CPTOOL_VERSION"),
    about = "Deterministic competitive-programming problem package tool",
    long_about = "cptool initializes problem packages, runs configured programs, generates official data, stress-tests solutions, checks package health, and exports judge data."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    #[command(about = "Create a minimal cptool/autocpp problem package")]
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
    #[command(
        about = "Stress test several programs on temporary generated inputs",
        long_about = "Stress test several programs on temporary generated inputs. Generator args after -- support {seed}, {case}, and {case0}; {case} is 1-based, {case0} is 0-based, and {seed} is deterministic."
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
            help = "Arguments passed to the generator after --; supports {seed}, {case}, and {case0}"
        )]
        args: Vec<String>,
    },
    #[command(
        about = "Run stress plans declared in problem.yaml",
        long_about = "Run stress plans declared in problem.yaml. Plan args support {seed}, {case}, and {case0}; {seed} is deterministic and may be controlled with stress.plans[].seed_base."
    )]
    StressPlan {
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
            help = "Reuse JSON from `stress-plan --summary-only --json` instead of rerunning stress plans"
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
    },
    #[command(about = "Export the package to an online judge format")]
    Export {
        #[arg(short, long, default_value = ".", help = "Problem package directory")]
        work_dir: PathBuf,
        #[arg(long, value_enum, help = "Target online judge format")]
        oj: OnlineJudge,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init { id, root } => {
            let path = tool::init_package(&root, &id)?;
            println!("created {}", path.display());
        }
        Commands::Run {
            program,
            case,
            work_dir,
            source,
            stdin_text,
            stdin_path,
            stdout_path,
            stderr_path,
            output_limit_bytes,
            wait_for_generation_lock,
            summary_only,
            json,
            hide_stdout,
            args,
        } => {
            let (program, selector) = normalize_run_positionals(program, case);
            let result = tool::run(RunOptions {
                work_dir,
                program,
                source,
                selector,
                stdin_text,
                stdin_path,
                stdout_path: stdout_path.clone(),
                stderr_path: stderr_path.clone(),
                args,
                output_limit_bytes,
                generation_lock_timeout: generation_lock_timeout(wait_for_generation_lock),
            })?;
            if json {
                print_json(&RunJsonSummary::from(&result))?;
            } else if summary_only {
                println!("{}", result.summary_line());
                if !result.ok {
                    eprintln!(
                        "hint: rerun without --summary-only or use --stdout-path/--stderr-path to save full output"
                    );
                }
            } else {
                println!("{}", result.status_line());
            }
            if !json
                && !summary_only
                && !hide_stdout
                && stdout_path.is_none()
                && !result.stdout.is_empty()
            {
                print!("{}", result.stdout);
            }
            if !json && !summary_only && stderr_path.is_none() && !result.stderr.is_empty() {
                eprint!("{}", result.stderr);
            }
            if !result.ok {
                std::process::exit(2);
            }
        }
        Commands::Gen {
            work_dir,
            bundle,
            case,
            output_dir,
            output_limit_bytes,
            wait_for_generation_lock,
            clean,
            summary_only,
            json,
        } => {
            let options = tool::GenerateOptions {
                work_dir,
                bundle,
                selector: case,
                output_dir,
                output_limit_bytes,
                clean,
                generation_lock_timeout: generation_lock_timeout(wait_for_generation_lock),
            };
            if json {
                let report = tool::generate_data_report_with_options(options)?;
                print_json(&report)?;
            } else if summary_only {
                let report = tool::generate_data_report_with_options(options)?;
                println!("{}", report.summary_line());
            } else {
                let generated = tool::generate_data_with_options(options)?;
                for path in generated {
                    println!("generated {}", path.display());
                }
            }
        }
        Commands::Stress {
            work_dir,
            generator,
            against,
            cases,
            output_limit_bytes,
            failure_dir,
            json,
            args,
        } => {
            if json {
                let summary = tool::stress_with_options(tool::StressOptions {
                    work_dir: &work_dir,
                    generator: &generator,
                    against: &against,
                    cases,
                    args: &args,
                    failure_dir: failure_dir.as_deref(),
                    output_limit_bytes,
                    print_progress: false,
                    print_warnings: false,
                })?;
                print_json(&StressJsonSummary::from(&summary))?;
            } else {
                let summary = tool::stress_with_summary(
                    &work_dir,
                    &generator,
                    &against,
                    cases,
                    &args,
                    failure_dir.as_deref(),
                    output_limit_bytes,
                )?;
                println!(
                    "stress passed: {} cases unique_input_hashes={}",
                    summary.cases, summary.unique_input_hashes
                );
            }
        }
        Commands::StressPlan {
            work_dir,
            name,
            output_limit_bytes,
            failure_dir,
            wait_for_generation_lock,
            summary_only,
            positive_only,
            negative_only,
            json,
        } => {
            let options = tool::StressPlanOptions {
                work_dir: &work_dir,
                name: name.as_deref(),
                failure_dir: failure_dir.as_deref(),
                output_limit_bytes,
                summary_only,
                filter: stress_plan_filter(positive_only, negative_only),
                generation_lock_timeout: generation_lock_timeout(wait_for_generation_lock),
            };
            if json {
                let summaries = tool::stress_plan_collect_with_options(options)?;
                let report = StressPlanJsonReport {
                    plans: summaries
                        .iter()
                        .map(StressJsonSummary::from)
                        .collect::<Vec<_>>(),
                };
                print_json(&report)?;
            } else {
                tool::stress_plan_with_options(options)?;
            }
        }
        Commands::Check {
            work_dir,
            wait_for_generation_lock,
            json,
        } => {
            let report = tool::check_problem_package_with_options(
                &work_dir,
                tool::CheckOptions {
                    generation_lock_timeout: generation_lock_timeout(wait_for_generation_lock),
                },
            );
            if json {
                print_json(&CheckJsonReport::from(&report))?;
            } else {
                print!("{}", report.render_text());
            }
            if report.has_errors() {
                std::process::exit(2);
            }
        }
        Commands::Evidence {
            work_dir,
            output_limit_bytes,
            skip_gen,
            skip_stress_plan,
            reuse_existing_stress_plan,
            wait_for_generation_lock,
            json,
        } => {
            let report = tool::collect_evidence(tool::EvidenceOptions {
                work_dir,
                output_limit_bytes,
                skip_gen,
                skip_stress_plan,
                reuse_existing_stress_plan,
                generation_lock_timeout: generation_lock_timeout(wait_for_generation_lock),
            });
            if json {
                print_json(&report)?;
            } else {
                print!("{}", report.render_text());
            }
            if report.has_errors() {
                std::process::exit(2);
            }
        }
        Commands::Export { work_dir, oj } => {
            let start = Instant::now();
            let work_dir = if work_dir.is_absolute() {
                work_dir
            } else {
                std::env::current_dir()?.join(work_dir)
            };
            let data_dir = work_dir.join("data");
            let problem = tool::load_problem(&work_dir)?;
            tool::generate_data(
                &work_dir,
                None,
                None,
                Some(&data_dir),
                DEFAULT_OUTPUT_LIMIT_BYTES,
            )?;

            match oj {
                OnlineJudge::Syzoj => {
                    let export_dir = work_dir.join("export").join("syzoj");
                    if export_dir.exists() {
                        std::fs::remove_dir_all(&export_dir)?;
                    }
                    std::fs::create_dir_all(&export_dir)?;
                    syzoj::SyzojExporter::export(&problem, &work_dir, &data_dir, &export_dir)?;
                    println!("exported {}", export_dir.display());
                }
            }
            let elapsed = start.elapsed();
            println!(
                "elapsed: {}.{:03}s",
                elapsed.as_secs(),
                elapsed.subsec_millis()
            );
        }
    }
    Ok(())
}

fn normalize_run_positionals(
    program: Option<String>,
    case: Option<String>,
) -> (Option<String>, Option<String>) {
    match (program, case) {
        (Some(first), None) if first.contains('[') && first.ends_with(']') => (None, Some(first)),
        (program, case) => (program, case),
    }
}

#[derive(Serialize)]
struct RunJsonSummary<'a> {
    label: &'a str,
    ok: bool,
    kind: &'a str,
    exit_code: Option<i32>,
    diagnostic: Option<&'a str>,
    elapsed_ms: u128,
    stdout_bytes: usize,
    stdout_lines: usize,
    stdout_sha256: String,
    stderr_bytes: usize,
    stderr_nonempty: bool,
    truncated_stdout: bool,
    truncated_stderr: bool,
}

impl<'a> From<&'a tool::RunResult> for RunJsonSummary<'a> {
    fn from(result: &'a tool::RunResult) -> Self {
        Self {
            label: &result.label,
            ok: result.ok,
            kind: &result.kind,
            exit_code: result.exit_code,
            diagnostic: result.diagnostic.as_deref(),
            elapsed_ms: result.elapsed_ms,
            stdout_bytes: result.stdout_bytes.len(),
            stdout_lines: count_lines(&result.stdout_bytes),
            stdout_sha256: format!("{:x}", Sha256::digest(&result.stdout_bytes)),
            stderr_bytes: result.stderr_bytes.len(),
            stderr_nonempty: !result.stderr_bytes.is_empty(),
            truncated_stdout: result.truncated_stdout,
            truncated_stderr: result.truncated_stderr,
        }
    }
}

#[derive(Serialize)]
struct StressPlanJsonReport<'a> {
    plans: Vec<StressJsonSummary<'a>>,
}

#[derive(Serialize)]
struct StressJsonSummary<'a> {
    plan_name: Option<&'a str>,
    cases: usize,
    elapsed_ms: u128,
    against: &'a [String],
    empty_stdout_cases: usize,
    all_empty_stdout_cases: usize,
    unique_input_hashes: usize,
    expected_failure: Option<&'a tool::ExpectedStressFailure>,
    warnings: Vec<JsonWarning>,
}

impl<'a> From<&'a tool::StressSummary> for StressJsonSummary<'a> {
    fn from(summary: &'a tool::StressSummary) -> Self {
        Self {
            plan_name: summary.plan_name.as_deref(),
            cases: summary.cases,
            elapsed_ms: summary.elapsed_ms,
            against: &summary.against,
            empty_stdout_cases: summary.empty_stdout_cases,
            all_empty_stdout_cases: summary.all_empty_stdout_cases,
            unique_input_hashes: summary.unique_input_hashes,
            expected_failure: summary.expected_failure.as_ref(),
            warnings: stress_warnings(summary),
        }
    }
}

#[derive(Serialize)]
struct CheckJsonReport<'a> {
    work_dir: &'a PathBuf,
    status: &'static str,
    errors: usize,
    warnings: usize,
    issues: &'a [tool::CheckIssue],
}

impl<'a> From<&'a tool::CheckReport> for CheckJsonReport<'a> {
    fn from(report: &'a tool::CheckReport) -> Self {
        Self {
            work_dir: &report.work_dir,
            status: if report.has_errors() { "fail" } else { "pass" },
            errors: report.error_count(),
            warnings: report.warning_count(),
            issues: &report.issues,
        }
    }
}

#[derive(Serialize)]
struct JsonWarning {
    code: &'static str,
    count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    random_coverage: Option<bool>,
}

fn stress_warnings(summary: &tool::StressSummary) -> Vec<JsonWarning> {
    let mut warnings = Vec::new();
    if summary.all_empty_stdout_cases > 0 {
        warnings.push(JsonWarning {
            code: "all_empty_output",
            count: summary.all_empty_stdout_cases,
            random_coverage: None,
        });
    }
    if summary.cases > 1 && summary.unique_input_hashes == 1 {
        warnings.push(JsonWarning {
            code: "repeated_input",
            count: 1,
            random_coverage: Some(false),
        });
    }
    warnings
}

fn stress_plan_filter(positive_only: bool, negative_only: bool) -> tool::StressPlanFilter {
    if positive_only {
        tool::StressPlanFilter::PositiveOnly
    } else if negative_only {
        tool::StressPlanFilter::NegativeOnly
    } else {
        tool::StressPlanFilter::All
    }
}

fn positive_seconds(value: &str) -> Result<u64, String> {
    let seconds = value
        .parse::<u64>()
        .map_err(|_| format!("`{value}` is not a positive integer number of seconds"))?;
    if seconds == 0 {
        return Err("value must be at least 1 second".to_string());
    }
    Ok(seconds)
}

fn generation_lock_timeout(seconds: Option<u64>) -> Option<Duration> {
    seconds.map(Duration::from_secs)
}

fn count_lines(bytes: &[u8]) -> usize {
    if bytes.is_empty() {
        0
    } else {
        bytes.iter().filter(|byte| **byte == b'\n').count() + usize::from(!bytes.ends_with(b"\n"))
    }
}

fn print_json<T: Serialize>(value: &T) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string(value)?);
    Ok(())
}
