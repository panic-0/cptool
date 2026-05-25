use anyhow::Context;
use clap::Parser;
use cptool::export::{Exporter, OnlineJudge, syzoj};
use cptool::tool::{self, DEFAULT_OUTPUT_LIMIT_BYTES, RunOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

mod args;
mod json;

use args::{
    AddCommands, AddProgramKindArg, AddTaskTypeArg, CaseCommands, Cli, Commands,
    FixtureAddCommands, FixtureCheckerCommands, FixtureCommands, FixtureValidatorCommands,
    JudgeExpectationArg, PkgCommands, ReportCommands, TestCommands,
};

pub fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Pkg { command } => handle_pkg(command)?,
        Commands::Add { command } => handle_add(command)?,
        Commands::Case { command } => handle_case(command)?,
        Commands::Test { command } => handle_test(command)?,
        Commands::Fixture { command } => handle_fixture(command)?,
        Commands::Report { command } => handle_report(command)?,
    }
    Ok(())
}

fn handle_pkg(command: PkgCommands) -> anyhow::Result<()> {
    match command {
        PkgCommands::Init { id, root } => handle_init(id, root)?,
        PkgCommands::Check {
            work_dir,
            wait_for_generation_lock,
            json,
        } => handle_check(work_dir, wait_for_generation_lock, json)?,
        PkgCommands::Clean {
            work_dir,
            data,
            cache,
            json,
        } => handle_clean(work_dir, data, cache, json)?,
        PkgCommands::Export { work_dir, oj } => handle_export(work_dir, oj)?,
    }
    Ok(())
}

fn handle_case(command: CaseCommands) -> anyhow::Result<()> {
    match command {
        CaseCommands::Gen {
            work_dir,
            bundle,
            case,
            output_dir,
            output_limit_bytes,
            wait_for_generation_lock,
            summary_only,
            json,
        } => handle_gen(GenCommandOptions {
            work_dir,
            bundle,
            case,
            output_dir,
            output_limit_bytes,
            wait_for_generation_lock,
            summary_only,
            json,
        })?,
        CaseCommands::Run {
            program,
            case,
            work_dir,
            source,
            stdin_text,
            stdin_path,
            stdout_path,
            stderr_path,
            output_limit_bytes,
            time_limit_secs,
            memory_limit_mb,
            wait_for_generation_lock,
            summary_only,
            json,
            hide_stdout,
            args,
        } => handle_run(RunCommandOptions {
            program,
            case,
            work_dir,
            source,
            stdin_text,
            stdin_path,
            stdout_path,
            stderr_path,
            output_limit_bytes,
            time_limit_secs,
            memory_limit_mb,
            wait_for_generation_lock,
            summary_only,
            json,
            hide_stdout,
            args,
        })?,
    }
    Ok(())
}

fn handle_test(command: TestCommands) -> anyhow::Result<()> {
    match command {
        TestCommands::Validator {
            work_dir,
            validator,
            input,
            fixture,
            expect,
            output_limit_bytes,
            no_fix_line_endings,
            line_ending_hints,
            json,
        } => handle_test_validator(TestValidatorCommandOptions {
            work_dir,
            validator,
            input,
            fixture,
            expect,
            output_limit_bytes,
            fix_line_endings: !no_fix_line_endings,
            line_ending_hints,
            json,
        })?,
        TestCommands::Checker {
            work_dir,
            checker,
            input,
            output,
            answer,
            fixture,
            expect,
            output_limit_bytes,
            json,
        } => handle_test_checker(TestCheckerCommandOptions {
            work_dir,
            checker,
            input,
            output,
            answer,
            fixture,
            expect,
            output_limit_bytes,
            json,
        })?,
        TestCommands::Stress {
            work_dir,
            generator,
            std,
            alt,
            answer,
            pass,
            fail,
            output_limit_bytes,
            failure_dir,
            json,
            args,
        } => handle_stress(StressCommandOptions {
            work_dir,
            generator,
            legacy_against: match (std, alt) {
                (Some(std), Some(alt)) => Some(vec![std, alt]),
                (None, None) => None,
                _ => anyhow::bail!(
                    "deprecated test stress positional mode requires both STD and ALT"
                ),
            },
            answer,
            pass,
            fail,
            output_limit_bytes,
            failure_dir,
            json,
            args,
        })?,
        TestCommands::Plan {
            work_dir,
            name,
            output_limit_bytes,
            failure_dir,
            wait_for_generation_lock,
            summary_only,
            positive_only,
            negative_only,
            json,
        } => handle_stress_plan(StressPlanCommandOptions {
            work_dir,
            name,
            output_limit_bytes,
            failure_dir,
            wait_for_generation_lock,
            summary_only,
            positive_only,
            negative_only,
            json,
        })?,
    }
    Ok(())
}

fn handle_fixture(command: FixtureCommands) -> anyhow::Result<()> {
    match command {
        FixtureCommands::Add { command } => handle_fixture_add(command)?,
        FixtureCommands::List { work_dir, json } => {
            let report = tool::list_fixtures(work_dir)?;
            print_fixture_list_report(report, json)?;
        }
        FixtureCommands::Check { work_dir, json } => {
            let report = tool::check_fixtures(work_dir)?;
            let ok = report.ok;
            print_fixture_check_report(report, json)?;
            if !ok {
                std::process::exit(2);
            }
        }
    }
    Ok(())
}

fn handle_fixture_add(command: FixtureAddCommands) -> anyhow::Result<()> {
    let report = match command {
        FixtureAddCommands::Input {
            name,
            work_dir,
            from,
            replace,
        } => tool::add_input_fixture(tool::AddInputFixtureOptions {
            work_dir,
            name,
            from,
            replace,
        })?,
        FixtureAddCommands::Validator { command } => {
            let (expect, name, work_dir, from, replace) = match command {
                FixtureValidatorCommands::Pass {
                    name,
                    work_dir,
                    from,
                    replace,
                } => (tool::JudgeExpectation::Pass, name, work_dir, from, replace),
                FixtureValidatorCommands::Fail {
                    name,
                    work_dir,
                    from,
                    replace,
                } => (tool::JudgeExpectation::Fail, name, work_dir, from, replace),
            };
            tool::add_validator_fixture(tool::AddValidatorFixtureOptions {
                work_dir,
                expect,
                name,
                from,
                replace,
            })?
        }
        FixtureAddCommands::Checker { command } => {
            let (expect, args) = fixture_checker_args(command);
            tool::add_checker_fixture(tool::AddCheckerFixtureOptions {
                work_dir: args.work_dir,
                expect,
                name: args.name,
                input_from: args.input,
                output_from: args.output,
                answer_from: args.answer,
                replace: args.replace,
            })?
        }
    };
    for line in report.summary_lines() {
        println!("{line}");
    }
    Ok(())
}

struct FixtureCheckerArgs {
    name: String,
    work_dir: PathBuf,
    input: Option<PathBuf>,
    output: Option<PathBuf>,
    answer: Option<PathBuf>,
    replace: bool,
}

fn fixture_checker_args(
    command: FixtureCheckerCommands,
) -> (tool::JudgeExpectation, FixtureCheckerArgs) {
    match command {
        FixtureCheckerCommands::Pass {
            name,
            work_dir,
            input,
            output,
            answer,
            replace,
        } => (
            tool::JudgeExpectation::Pass,
            FixtureCheckerArgs {
                name,
                work_dir,
                input,
                output,
                answer,
                replace,
            },
        ),
        FixtureCheckerCommands::Fail {
            name,
            work_dir,
            input,
            output,
            answer,
            replace,
        } => (
            tool::JudgeExpectation::Fail,
            FixtureCheckerArgs {
                name,
                work_dir,
                input,
                output,
                answer,
                replace,
            },
        ),
    }
}

fn print_fixture_list_report(report: tool::FixtureListReport, json: bool) -> anyhow::Result<()> {
    let report = display_fixture_list_report(report);
    if json {
        self::json::print(&report)?;
    } else {
        println!(
            "fixtures: inputs={} validators={} checkers={}",
            report.inputs.len(),
            report.validators.len(),
            report.checkers.len()
        );
        for input in &report.inputs {
            println!(
                "input {} used={} path={}",
                input.name,
                input.used,
                input.path.display()
            );
        }
        for validator in &report.validators {
            println!(
                "validator {}/{} path={}",
                validator.expect.as_str(),
                validator.name,
                validator.path.display()
            );
        }
        for checker in &report.checkers {
            println!(
                "checker {}/{} path={}",
                checker.expect.as_str(),
                checker.name,
                checker.path.display()
            );
        }
    }
    Ok(())
}

fn print_fixture_check_report(report: tool::FixtureCheckReport, json: bool) -> anyhow::Result<()> {
    let report = display_fixture_check_report(report);
    if json {
        self::json::print(&report)?;
    } else {
        println!(
            "fixture check: {} errors={}",
            if report.ok { "ok" } else { "fail" },
            report.errors.len()
        );
        for issue in &report.errors {
            eprintln!("{} {}: {}", issue.code, issue.path.display(), issue.message);
        }
    }
    Ok(())
}

fn handle_report(command: ReportCommands) -> anyhow::Result<()> {
    match command {
        ReportCommands::Evidence {
            work_dir,
            output_limit_bytes,
            skip_gen,
            skip_stress_plan,
            reuse_existing_stress_plan,
            wait_for_generation_lock,
            json,
            markdown,
            out,
        } => handle_evidence(EvidenceCommandOptions {
            work_dir,
            output_limit_bytes,
            skip_gen,
            skip_stress_plan,
            reuse_existing_stress_plan,
            wait_for_generation_lock,
            json,
            markdown,
            out,
        })?,
    }
    Ok(())
}

fn handle_init(id: String, root: PathBuf) -> anyhow::Result<()> {
    let path = tool::init_package(&root, &id)?;
    println!("created {}", terminal_path(&path, None));
    Ok(())
}

struct RunCommandOptions {
    program: Option<String>,
    case: Option<String>,
    work_dir: PathBuf,
    source: Option<PathBuf>,
    stdin_text: Option<String>,
    stdin_path: Option<PathBuf>,
    stdout_path: Option<PathBuf>,
    stderr_path: Option<PathBuf>,
    output_limit_bytes: usize,
    time_limit_secs: Option<f64>,
    memory_limit_mb: Option<f64>,
    wait_for_generation_lock: Option<u64>,
    summary_only: bool,
    json: bool,
    hide_stdout: bool,
    args: Vec<String>,
}

fn handle_run(options: RunCommandOptions) -> anyhow::Result<()> {
    let RunCommandOptions {
        program,
        case,
        work_dir,
        source,
        stdin_text,
        stdin_path,
        stdout_path,
        stderr_path,
        output_limit_bytes,
        time_limit_secs,
        memory_limit_mb,
        wait_for_generation_lock,
        summary_only,
        json,
        hide_stdout,
        args,
    } = options;
    let (program, selector) = normalize_run_positionals(program, case);
    let result = match tool::run(RunOptions {
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
        time_limit_secs,
        memory_limit_mb,
    }) {
        Ok(result) => result,
        Err(err) => {
            if let Some(failure) = err.downcast_ref::<tool::CompileFailure>() {
                let result = failure.result.as_ref();
                if json {
                    self::json::print(&self::json::RunJsonSummary::from(result))?;
                } else {
                    println!(
                        "{} {}",
                        result.result_line(),
                        result.compile.summary_fragment()
                    );
                    if !result.stderr.is_empty() {
                        eprint!("{}", result.stderr);
                    }
                }
                std::process::exit(2);
            }
            return Err(err);
        }
    };
    if json {
        self::json::print(&self::json::RunJsonSummary::from(&result))?;
    } else if summary_only {
        println!("{}", result.summary_line());
        if !result.is_success() {
            eprintln!(
                "hint: rerun without --summary-only or use --stdout-path/--stderr-path to save full output"
            );
        }
    } else {
        println!(
            "{} {}",
            result.result_line(),
            result.compile.summary_fragment()
        );
    }
    if !json && !summary_only && !hide_stdout && stdout_path.is_none() && !result.stdout.is_empty()
    {
        print!("{}", result.stdout);
    }
    if !json && !summary_only && stderr_path.is_none() && !result.stderr.is_empty() {
        eprint!("{}", result.stderr);
    }
    if !result.is_success() {
        std::process::exit(2);
    }
    Ok(())
}

fn unwrap_or_print_compile_failure<T>(result: anyhow::Result<T>, json: bool) -> anyhow::Result<T> {
    match result {
        Ok(value) => Ok(value),
        Err(err) => {
            if let Some(failure) = err.downcast_ref::<tool::CompileFailure>() {
                print_compile_failure(failure, json)?;
                std::process::exit(2);
            }
            Err(err)
        }
    }
}

fn print_compile_failure(failure: &tool::CompileFailure, json: bool) -> anyhow::Result<()> {
    let result = failure.result.as_ref();
    if json {
        self::json::print(&self::json::RunJsonSummary::from(result))?;
    } else {
        println!(
            "{} {}",
            result.result_line(),
            result.compile.summary_fragment()
        );
        if !result.stderr.is_empty() {
            eprint!("{}", result.stderr);
        }
    }
    Ok(())
}

struct GenCommandOptions {
    work_dir: PathBuf,
    bundle: Option<String>,
    case: Option<String>,
    output_dir: Option<PathBuf>,
    output_limit_bytes: usize,
    wait_for_generation_lock: Option<u64>,
    summary_only: bool,
    json: bool,
}

fn handle_gen(options: GenCommandOptions) -> anyhow::Result<()> {
    let GenCommandOptions {
        work_dir,
        bundle,
        case,
        output_dir,
        output_limit_bytes,
        wait_for_generation_lock,
        summary_only,
        json,
    } = options;
    let display_work_dir = work_dir.clone();
    let options = tool::GenerateOptions {
        work_dir,
        bundle,
        selector: case,
        output_dir,
        output_limit_bytes,
        generation_lock_timeout: generation_lock_timeout(wait_for_generation_lock),
    };
    if json {
        let report = unwrap_or_print_compile_failure(
            tool::generate_data_report_with_options(options),
            json,
        )?;
        self::json::print(&display_generate_report(report, &display_work_dir))?;
    } else if summary_only {
        let report = unwrap_or_print_compile_failure(
            tool::generate_data_report_with_options(options),
            json,
        )?;
        println!("{}", report.summary_line());
    } else {
        let generated =
            unwrap_or_print_compile_failure(tool::generate_data_with_options(options), json)?;
        for path in generated {
            println!(
                "generated {}",
                terminal_path(&path, Some(&display_work_dir))
            );
        }
    }
    Ok(())
}

fn handle_clean(work_dir: PathBuf, data: bool, cache: bool, json: bool) -> anyhow::Result<()> {
    let report = tool::clean_package_with_options(tool::CleanOptions {
        work_dir,
        data,
        cache,
    })?;
    if json {
        self::json::print(&display_clean_report(report))?;
    } else {
        println!("{}", report.summary_line());
    }
    Ok(())
}

struct StressCommandOptions {
    work_dir: PathBuf,
    generator: String,
    legacy_against: Option<Vec<String>>,
    answer: Option<String>,
    pass: Vec<String>,
    fail: Vec<String>,
    output_limit_bytes: usize,
    failure_dir: Option<PathBuf>,
    json: bool,
    args: Vec<String>,
}

fn handle_stress(options: StressCommandOptions) -> anyhow::Result<()> {
    if let Some(against) = options.legacy_against {
        eprintln!(
            "warning: deprecated test stress positional mode; use --answer {} --pass {}",
            against[0], against[1]
        );
        return handle_legacy_stress(StressCommandOptions {
            legacy_against: Some(against),
            ..options
        });
    }
    let answer = options.answer.unwrap_or_default();
    let args_by_case = tool::range_args(&options.args)?;
    let summaries = unwrap_or_print_compile_failure(
        tool::stress_expect_with_options(tool::StressExpectOptions {
            work_dir: &options.work_dir,
            generator: &options.generator,
            answer: &answer,
            pass_programs: &options.pass,
            fail_programs: &options.fail,
            args_by_case,
            failure_dir: options.failure_dir.as_deref(),
            output_limit_bytes: options.output_limit_bytes,
            print_progress: !options.json,
            print_warnings: !options.json,
        }),
        options.json,
    )?;
    if options.json {
        let summaries = display_stress_summaries(summaries, &options.work_dir);
        let report = self::json::StressPlanJsonReport::from_summaries(&summaries);
        self::json::print(&report)?;
    } else {
        let cases = summaries
            .iter()
            .map(|summary| summary.cases)
            .max()
            .unwrap_or(0);
        println!(
            "stress expect passed: {} cases checks={}",
            cases,
            summaries.len()
        );
    }
    Ok(())
}

fn handle_legacy_stress(options: StressCommandOptions) -> anyhow::Result<()> {
    let against = options
        .legacy_against
        .as_ref()
        .expect("legacy stress requires positional programs");
    if options.json {
        let summary = unwrap_or_print_compile_failure(
            tool::stress_with_options(tool::StressOptions {
                work_dir: &options.work_dir,
                generator: &options.generator,
                against,
                cases: 1,
                args: &options.args,
                failure_dir: options.failure_dir.as_deref(),
                output_limit_bytes: options.output_limit_bytes,
                print_progress: false,
                print_warnings: false,
            }),
            options.json,
        )?;
        let summary = display_stress_summary(summary, &options.work_dir);
        self::json::print(&self::json::StressJsonSummary::from(&summary))?;
    } else {
        let summary = unwrap_or_print_compile_failure(
            tool::stress_with_summary(
                &options.work_dir,
                &options.generator,
                against,
                1,
                &options.args,
                options.failure_dir.as_deref(),
                options.output_limit_bytes,
            ),
            options.json,
        )?;
        println!(
            "stress passed: {} cases unique_input_hashes={}",
            summary.cases, summary.unique_input_hashes
        );
    }
    Ok(())
}

struct StressPlanCommandOptions {
    work_dir: PathBuf,
    name: Option<String>,
    output_limit_bytes: usize,
    failure_dir: Option<PathBuf>,
    wait_for_generation_lock: Option<u64>,
    summary_only: bool,
    positive_only: bool,
    negative_only: bool,
    json: bool,
}

fn handle_stress_plan(options: StressPlanCommandOptions) -> anyhow::Result<()> {
    let stress_options = tool::StressPlanOptions {
        work_dir: &options.work_dir,
        name: options.name.as_deref(),
        failure_dir: options.failure_dir.as_deref(),
        output_limit_bytes: options.output_limit_bytes,
        summary_only: options.summary_only,
        filter: stress_plan_filter(options.positive_only, options.negative_only),
        generation_lock_timeout: generation_lock_timeout(options.wait_for_generation_lock),
    };
    if options.json {
        let summaries = unwrap_or_print_compile_failure(
            tool::stress_plan_collect_with_options(stress_options),
            options.json,
        )?;
        let summaries = display_stress_summaries(summaries, &options.work_dir);
        let report = self::json::StressPlanJsonReport::from_summaries(&summaries);
        self::json::print(&report)?;
    } else {
        unwrap_or_print_compile_failure(
            tool::stress_plan_with_options(stress_options),
            options.json,
        )?;
    }
    Ok(())
}

fn handle_check(
    work_dir: PathBuf,
    wait_for_generation_lock: Option<u64>,
    json: bool,
) -> anyhow::Result<()> {
    let report = tool::check_problem_package_with_options(
        &work_dir,
        tool::CheckOptions {
            generation_lock_timeout: generation_lock_timeout(wait_for_generation_lock),
        },
    );
    if json {
        let display_report = display_check_report(report.clone());
        self::json::print(&self::json::CheckJsonReport::from(&display_report))?;
    } else {
        print!("{}", display_check_report(report.clone()).render_text());
    }
    if report.has_errors() {
        std::process::exit(2);
    }
    Ok(())
}

struct EvidenceCommandOptions {
    work_dir: PathBuf,
    output_limit_bytes: usize,
    skip_gen: bool,
    skip_stress_plan: bool,
    reuse_existing_stress_plan: Option<PathBuf>,
    wait_for_generation_lock: Option<u64>,
    json: bool,
    markdown: bool,
    out: Option<PathBuf>,
}

fn handle_evidence(options: EvidenceCommandOptions) -> anyhow::Result<()> {
    let report = tool::collect_evidence(tool::EvidenceOptions {
        work_dir: options.work_dir,
        output_limit_bytes: options.output_limit_bytes,
        skip_gen: options.skip_gen,
        skip_stress_plan: options.skip_stress_plan,
        reuse_existing_stress_plan: options.reuse_existing_stress_plan,
        generation_lock_timeout: generation_lock_timeout(options.wait_for_generation_lock),
    });
    let has_errors = report.has_errors();
    let display_report = display_evidence_report(report);
    if options.json {
        let output = self::json::to_bytes(&display_report)?;
        write_optional_sidecar(options.out.as_deref(), &output)?;
        std::io::stdout().lock().write_all(&output)?;
    } else if options.markdown {
        let output = display_report.render_quality_markdown().into_bytes();
        write_optional_sidecar(options.out.as_deref(), &output)?;
        std::io::stdout().lock().write_all(&output)?;
    } else {
        let output = display_report.render_text().into_bytes();
        write_optional_sidecar(options.out.as_deref(), &output)?;
        std::io::stdout().lock().write_all(&output)?;
    }
    if has_errors {
        std::process::exit(2);
    }
    Ok(())
}

fn write_optional_sidecar(path: Option<&Path>, bytes: &[u8]) -> anyhow::Result<()> {
    let Some(path) = path else {
        return Ok(());
    };
    write_sidecar_atomic(path, bytes)
}

fn write_sidecar_atomic(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    use anyhow::Context;

    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create evidence output parent dir {}",
                parent.display()
            )
        })?;
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("evidence");
    let temp_path = parent.join(format!(".{file_name}.{}.tmp", sidecar_temp_suffix()));
    let write_result = (|| -> anyhow::Result<()> {
        std::fs::write(&temp_path, bytes).with_context(|| {
            format!(
                "failed to write evidence output temp file {}",
                temp_path.display()
            )
        })?;
        std::fs::rename(&temp_path, path).with_context(|| {
            format!(
                "failed to move evidence output temp file {} to {}",
                temp_path.display(),
                path.display()
            )
        })?;
        Ok(())
    })();
    if write_result.is_err() {
        let _ = std::fs::remove_file(&temp_path);
    }
    write_result
}

fn sidecar_temp_suffix() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("{}-{nanos}", std::process::id())
}

fn handle_export(work_dir: PathBuf, oj: OnlineJudge) -> anyhow::Result<()> {
    let start = Instant::now();
    let work_dir = if work_dir.is_absolute() {
        work_dir
    } else {
        std::env::current_dir()?.join(work_dir)
    };
    let data_dir = work_dir.join("data");
    let problem = tool::load_problem(&work_dir)?;
    tool::generate_data_with_options(tool::GenerateOptions {
        work_dir: work_dir.clone(),
        bundle: None,
        selector: None,
        output_dir: Some(data_dir.clone()),
        output_limit_bytes: DEFAULT_OUTPUT_LIMIT_BYTES,
        generation_lock_timeout: None,
    })?;

    match oj {
        OnlineJudge::Syzoj => {
            let export_dir = work_dir.join("export").join("syzoj");
            if export_dir.exists() {
                std::fs::remove_dir_all(&export_dir)?;
            }
            std::fs::create_dir_all(&export_dir)?;
            syzoj::SyzojExporter::export(&problem, &work_dir, &data_dir, &export_dir)?;
            println!("exported {}", terminal_path(&export_dir, Some(&work_dir)));
        }
    }
    let elapsed = start.elapsed();
    println!(
        "elapsed: {}.{:03}s",
        elapsed.as_secs(),
        elapsed.subsec_millis()
    );
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

fn stress_plan_filter(positive_only: bool, negative_only: bool) -> tool::StressPlanFilter {
    if positive_only {
        tool::StressPlanFilter::PositiveOnly
    } else if negative_only {
        tool::StressPlanFilter::NegativeOnly
    } else {
        tool::StressPlanFilter::All
    }
}

fn generation_lock_timeout(seconds: Option<u64>) -> Option<Duration> {
    seconds.map(Duration::from_secs)
}

fn display_generate_report(
    mut report: tool::GenerateReport,
    work_dir: &Path,
) -> tool::GenerateReport {
    report.paths = report
        .paths
        .into_iter()
        .map(|path| PathBuf::from(terminal_path(&path, Some(work_dir))))
        .collect();
    report
}

fn display_clean_report(mut report: tool::CleanReport) -> tool::CleanReport {
    let original_work_dir = report.work_dir.clone();
    report.work_dir = PathBuf::from(terminal_path(&original_work_dir, None));
    report.paths_removed = report
        .paths_removed
        .into_iter()
        .map(|path| PathBuf::from(terminal_path(&path, Some(&original_work_dir))))
        .collect();
    report
}

fn display_check_report(mut display_report: tool::CheckReport) -> tool::CheckReport {
    let original_work_dir = display_report.work_dir.clone();
    let display_work_dir = terminal_path(&original_work_dir, None);

    display_report.work_dir = PathBuf::from(&display_work_dir);
    shorten_check_issues(
        &mut display_report.issues,
        &original_work_dir,
        &display_work_dir,
    );
    display_report
}

fn display_evidence_report(mut report: tool::EvidenceReport) -> tool::EvidenceReport {
    let original_work_dir = report.work_dir.clone();
    report.work_dir = PathBuf::from(terminal_path(&original_work_dir, None));

    if let Some(check) = report.check.report.as_mut() {
        let original_check_work_dir = check.work_dir.clone();
        let display_check_work_dir = terminal_path(&original_check_work_dir, None);
        check.work_dir = PathBuf::from(&display_check_work_dir);
        shorten_check_issues(
            &mut check.issues,
            &original_check_work_dir,
            &display_check_work_dir,
        );
    }

    if let Some(gen_report) = report.r#gen.report.as_mut() {
        gen_report.paths = gen_report
            .paths
            .iter()
            .map(|path| PathBuf::from(terminal_path(path, Some(&original_work_dir))))
            .collect();
    }

    if let Some(stress_reports) = report.stress_plan.report.as_mut() {
        for summary in stress_reports {
            shorten_stress_summary_paths(summary, &original_work_dir);
        }
    }

    report
}

fn display_stress_summaries(
    mut summaries: Vec<tool::StressSummary>,
    work_dir: &Path,
) -> Vec<tool::StressSummary> {
    for summary in &mut summaries {
        shorten_stress_summary_paths(summary, work_dir);
    }
    summaries
}

fn display_stress_summary(
    mut summary: tool::StressSummary,
    work_dir: &Path,
) -> tool::StressSummary {
    shorten_stress_summary_paths(&mut summary, work_dir);
    summary
}

fn display_judge_report(
    mut report: tool::JudgeReport,
    work_dir: Option<&Path>,
) -> tool::JudgeReport {
    if let Some(path) = &report.report_path {
        report.report_path = Some(PathBuf::from(terminal_path(path, work_dir)));
    }
    report
}

fn display_fixture_list_report(mut report: tool::FixtureListReport) -> tool::FixtureListReport {
    let original_work_dir = report.work_dir.clone();
    report.work_dir = PathBuf::from(terminal_path(&original_work_dir, None));
    shorten_fixture_list_paths(&mut report, &original_work_dir);
    report
}

fn display_fixture_check_report(mut report: tool::FixtureCheckReport) -> tool::FixtureCheckReport {
    let original_work_dir = report.work_dir.clone();
    report.work_dir = PathBuf::from(terminal_path(&original_work_dir, None));
    shorten_fixture_list_paths(&mut report.list, &original_work_dir);
    for issue in &mut report.errors {
        issue.path = PathBuf::from(terminal_path(&issue.path, Some(&original_work_dir)));
    }
    report
}

fn shorten_fixture_list_paths(report: &mut tool::FixtureListReport, work_dir: &Path) {
    for input in &mut report.inputs {
        input.path = PathBuf::from(terminal_path(&input.path, Some(work_dir)));
    }
    for validator in &mut report.validators {
        validator.path = PathBuf::from(terminal_path(&validator.path, Some(work_dir)));
    }
    for checker in &mut report.checkers {
        checker.path = PathBuf::from(terminal_path(&checker.path, Some(work_dir)));
        checker.input_path = PathBuf::from(terminal_path(&checker.input_path, Some(work_dir)));
        checker.output_path = PathBuf::from(terminal_path(&checker.output_path, Some(work_dir)));
        checker.answer_path = PathBuf::from(terminal_path(&checker.answer_path, Some(work_dir)));
    }
}

fn shorten_check_issues(issues: &mut [tool::CheckIssue], work_dir: &Path, display_work_dir: &str) {
    for issue in issues {
        if let Some(path) = &issue.path {
            issue.path = Some(PathBuf::from(terminal_path(path, Some(work_dir))));
        }
        if let Some(next_action) = &mut issue.next_action {
            let original = work_dir.display().to_string();
            *next_action = next_action.replace(&original, display_work_dir);
        }
    }
}

fn shorten_stress_summary_paths(summary: &mut tool::StressSummary, work_dir: &Path) {
    if let Some(failure) = summary.expected_failure.as_mut() {
        failure.input_path = PathBuf::from(terminal_path(&failure.input_path, Some(work_dir)));
        failure.report_path = PathBuf::from(terminal_path(&failure.report_path, Some(work_dir)));
        for output in &mut failure.outputs {
            output.stdout_path = PathBuf::from(terminal_path(&output.stdout_path, Some(work_dir)));
            output.stderr_path = PathBuf::from(terminal_path(&output.stderr_path, Some(work_dir)));
        }
        if let Some(checker) = failure.checker.as_mut() {
            checker.stdout_path =
                PathBuf::from(terminal_path(&checker.stdout_path, Some(work_dir)));
            checker.stderr_path =
                PathBuf::from(terminal_path(&checker.stderr_path, Some(work_dir)));
            checker.report_path = checker
                .report_path
                .as_ref()
                .map(|path| PathBuf::from(terminal_path(path, Some(work_dir))));
        }
    }
}

fn terminal_path(path: &Path, primary_base: Option<&Path>) -> String {
    let path_abs = absolutize_for_display(path);
    if let Some(base) = primary_base {
        let base_abs = absolutize_for_display(base);
        if let Ok(relative) = path_abs.strip_prefix(&base_abs) {
            return path_to_terminal_string(relative);
        }
    }

    if let Ok(cwd) = std::env::current_dir()
        && let Ok(relative) = path_abs.strip_prefix(&cwd)
    {
        return path_to_terminal_string(relative);
    }

    path_to_terminal_string(path)
}

fn absolutize_for_display(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    }
}

fn path_to_terminal_string(path: &Path) -> String {
    if path.as_os_str().is_empty() {
        ".".to_string()
    } else {
        path.display().to_string().replace('\\', "/")
    }
}

struct TestValidatorCommandOptions {
    work_dir: PathBuf,
    validator: Option<String>,
    input: Option<PathBuf>,
    fixture: Option<String>,
    expect: JudgeExpectationArg,
    output_limit_bytes: usize,
    fix_line_endings: bool,
    line_ending_hints: bool,
    json: bool,
}

fn handle_test_validator(options: TestValidatorCommandOptions) -> anyhow::Result<()> {
    match (options.input, options.fixture) {
        (Some(input), None) => {
            let display_work_dir = options.work_dir.clone();
            let report = unwrap_or_print_compile_failure(
                tool::judge_validator(tool::JudgeValidatorOptions {
                    work_dir: options.work_dir,
                    validator: options.validator,
                    input_path: input,
                    expect: convert_judge_expectation(options.expect),
                    output_limit_bytes: options.output_limit_bytes,
                    fix_line_endings: options.fix_line_endings,
                    line_ending_hints: options.line_ending_hints,
                }),
                options.json,
            )?;
            print_judge_report(&report, options.json, Some(&display_work_dir))?;
            if !report.ok {
                std::process::exit(2);
            }
        }
        (None, Some(selector)) => {
            let fixture = select_validator_fixture(options.work_dir.clone(), &selector)?;
            let ok = run_validator_fixture_batch(
                options.work_dir,
                options.validator,
                vec![fixture],
                options.output_limit_bytes,
                options.fix_line_endings,
                options.line_ending_hints,
                options.json,
            )?;
            if !ok {
                std::process::exit(2);
            }
        }
        (None, None) => {
            let fixtures = tool::validator_fixture_reports(options.work_dir.clone())?;
            if fixtures.is_empty() {
                anyhow::bail!(
                    "no validator fixtures found under fixtures/validator/pass or fixtures/validator/fail"
                );
            }
            let ok = run_validator_fixture_batch(
                options.work_dir,
                options.validator,
                fixtures,
                options.output_limit_bytes,
                options.fix_line_endings,
                options.line_ending_hints,
                options.json,
            )?;
            if !ok {
                std::process::exit(2);
            }
        }
        (Some(_), Some(_)) => {
            anyhow::bail!("pass either --input or --fixture, not both");
        }
    }
    Ok(())
}

fn run_validator_fixture_batch(
    work_dir: PathBuf,
    validator: Option<String>,
    fixtures: Vec<tool::ValidatorFixture>,
    output_limit_bytes: usize,
    fix_line_endings: bool,
    line_ending_hints: bool,
    json: bool,
) -> anyhow::Result<bool> {
    let display_work_dir = work_dir.clone();
    let mut reports = Vec::new();
    for fixture in fixtures {
        let report = unwrap_or_print_compile_failure(
            tool::judge_validator(tool::JudgeValidatorOptions {
                work_dir: work_dir.clone(),
                validator: validator.clone(),
                input_path: fixture.path.clone(),
                expect: fixture.expect,
                output_limit_bytes,
                fix_line_endings,
                line_ending_hints,
            }),
            json,
        )?;
        reports.push((fixture, report));
    }
    print_judge_fixture_batch("validator", &reports, json, &display_work_dir)?;
    Ok(reports.iter().all(|(_, report)| report.ok))
}

fn select_validator_fixture(
    work_dir: PathBuf,
    selector: &str,
) -> anyhow::Result<tool::ValidatorFixture> {
    let (expect, name) = parse_fixture_selector(selector)?;
    let fixtures = tool::validator_fixture_reports(work_dir)?;
    fixtures
        .into_iter()
        .find(|fixture| fixture.expect == expect && fixture.name == name)
        .with_context(|| format!("validator fixture `{selector}` not found"))
}

fn print_judge_fixture_batch<F: FixtureDisplay>(
    role: &'static str,
    reports: &[(F, tool::JudgeReport)],
    json: bool,
    display_work_dir: &Path,
) -> anyhow::Result<()> {
    if json {
        let display_reports: Vec<(String, PathBuf, tool::JudgeReport)> = reports
            .iter()
            .map(|(fixture, report)| {
                (
                    fixture.fixture_name().to_string(),
                    PathBuf::from(terminal_path(
                        fixture.fixture_path(),
                        Some(display_work_dir),
                    )),
                    display_judge_report(report.clone(), Some(display_work_dir)),
                )
            })
            .collect();
        let fixtures = display_reports
            .iter()
            .map(|(name, path, report)| self::json::JudgeFixtureJsonSummary {
                name,
                path,
                report: self::json::JudgeJsonSummary::from(report),
            })
            .collect();
        self::json::print(&self::json::JudgeBatchJsonSummary::new(role, fixtures))?;
    } else {
        for (fixture, report) in reports {
            for warning in &report.warnings {
                eprintln!("warning: {} {}", warning.code, warning.message);
            }
            println!(
                "{} {}: {}",
                role,
                terminal_path(fixture.fixture_path(), Some(display_work_dir)),
                report.summary_line()
            );
        }
        let passed = reports.iter().filter(|(_, report)| report.ok).count();
        println!(
            "{} fixtures: total={} passed={} failed={}",
            role,
            reports.len(),
            passed,
            reports.len() - passed
        );
    }
    Ok(())
}

trait FixtureDisplay {
    fn fixture_name(&self) -> &str;
    fn fixture_path(&self) -> &Path;
}

impl FixtureDisplay for tool::ValidatorFixture {
    fn fixture_name(&self) -> &str {
        &self.name
    }

    fn fixture_path(&self) -> &Path {
        &self.path
    }
}

impl FixtureDisplay for tool::CheckerFixture {
    fn fixture_name(&self) -> &str {
        &self.name
    }

    fn fixture_path(&self) -> &Path {
        &self.path
    }
}

fn parse_fixture_selector(selector: &str) -> anyhow::Result<(tool::JudgeExpectation, String)> {
    let Some((expect, name)) = selector.split_once('/') else {
        anyhow::bail!("fixture selector must look like pass/name or fail/name");
    };
    let expect = match expect {
        "pass" => tool::JudgeExpectation::Pass,
        "fail" => tool::JudgeExpectation::Fail,
        _ => anyhow::bail!("fixture selector must start with pass/ or fail/"),
    };
    if name.is_empty() || name.contains('/') || name.contains('\\') {
        anyhow::bail!("fixture selector name must be a single fixture name");
    }
    Ok((expect, name.to_string()))
}

struct TestCheckerCommandOptions {
    work_dir: PathBuf,
    checker: Option<String>,
    input: Option<PathBuf>,
    output: Option<PathBuf>,
    answer: Option<PathBuf>,
    fixture: Option<String>,
    expect: JudgeExpectationArg,
    output_limit_bytes: usize,
    json: bool,
}

fn handle_test_checker(options: TestCheckerCommandOptions) -> anyhow::Result<()> {
    let explicit_count = usize::from(options.input.is_some())
        + usize::from(options.output.is_some())
        + usize::from(options.answer.is_some());
    if options.fixture.is_some() && explicit_count > 0 {
        anyhow::bail!("pass either --fixture or --input/--output/--answer, not both");
    }
    if explicit_count > 0 && explicit_count < 3 {
        let mut missing = Vec::new();
        if options.input.is_none() {
            missing.push("--input");
        }
        if options.output.is_none() {
            missing.push("--output");
        }
        if options.answer.is_none() {
            missing.push("--answer");
        }
        anyhow::bail!("explicit checker mode is missing {}", missing.join(" and "));
    }

    match (
        options.input,
        options.output,
        options.answer,
        options.fixture,
    ) {
        (Some(input), Some(output), Some(answer), None) => {
            let display_work_dir = options.work_dir.clone();
            let report = unwrap_or_print_compile_failure(
                tool::judge_checker(tool::JudgeCheckerOptions {
                    work_dir: options.work_dir,
                    checker: options.checker,
                    input_path: input,
                    output_path: output,
                    answer_path: answer,
                    expect: convert_judge_expectation(options.expect),
                    output_limit_bytes: options.output_limit_bytes,
                }),
                options.json,
            )?;
            print_judge_report(&report, options.json, Some(&display_work_dir))?;
            if !report.ok {
                std::process::exit(2);
            }
        }
        (None, None, None, Some(selector)) => {
            let fixture = select_checker_fixture(options.work_dir.clone(), &selector)?;
            let ok = run_checker_fixture_batch(
                options.work_dir,
                options.checker,
                vec![fixture],
                options.output_limit_bytes,
                options.json,
            )?;
            if !ok {
                std::process::exit(2);
            }
        }
        (None, None, None, None) => {
            let fixtures = tool::checker_fixture_reports(options.work_dir.clone())?;
            if fixtures.is_empty() {
                anyhow::bail!(
                    "no checker fixtures found under fixtures/checker/pass or fixtures/checker/fail"
                );
            }
            let ok = run_checker_fixture_batch(
                options.work_dir,
                options.checker,
                fixtures,
                options.output_limit_bytes,
                options.json,
            )?;
            if !ok {
                std::process::exit(2);
            }
        }
        _ => unreachable!("checker mode was validated before dispatch"),
    }
    Ok(())
}

fn run_checker_fixture_batch(
    work_dir: PathBuf,
    checker: Option<String>,
    fixtures: Vec<tool::CheckerFixture>,
    output_limit_bytes: usize,
    json: bool,
) -> anyhow::Result<bool> {
    let display_work_dir = work_dir.clone();
    let mut reports = Vec::new();
    for fixture in fixtures {
        let report = unwrap_or_print_compile_failure(
            tool::judge_checker(tool::JudgeCheckerOptions {
                work_dir: work_dir.clone(),
                checker: checker.clone(),
                input_path: fixture.input_path.clone(),
                output_path: fixture.output_path.clone(),
                answer_path: fixture.answer_path.clone(),
                expect: fixture.expect,
                output_limit_bytes,
            }),
            json,
        )?;
        reports.push((fixture, report));
    }
    print_judge_fixture_batch("checker", &reports, json, &display_work_dir)?;
    Ok(reports.iter().all(|(_, report)| report.ok))
}

fn select_checker_fixture(
    work_dir: PathBuf,
    selector: &str,
) -> anyhow::Result<tool::CheckerFixture> {
    let (expect, name) = parse_fixture_selector(selector)?;
    let fixtures = tool::checker_fixture_reports(work_dir)?;
    fixtures
        .into_iter()
        .find(|fixture| fixture.expect == expect && fixture.name == name)
        .with_context(|| format!("checker fixture `{selector}` not found"))
}

fn print_judge_report(
    report: &tool::JudgeReport,
    json: bool,
    display_work_dir: Option<&Path>,
) -> anyhow::Result<()> {
    if json {
        let display_report = display_judge_report(report.clone(), display_work_dir);
        self::json::print(&self::json::JudgeJsonSummary::from(&display_report))?;
    } else {
        for warning in &report.warnings {
            eprintln!("warning: {} {}", warning.code, warning.message);
        }
        println!("{}", report.summary_line());
        if !report.ok {
            eprintln!(
                "expected {}, observed {}",
                report.expect.as_str(),
                report.observed.as_str()
            );
        }
    }
    Ok(())
}

fn handle_add(command: AddCommands) -> anyhow::Result<()> {
    match command {
        AddCommands::Program {
            name,
            work_dir,
            kind,
            path,
            time_limit_secs,
            memory_limit_mb,
            compile_arg,
            replace,
        } => tool::add_program(tool::AddProgramOptions {
            work_dir,
            name,
            kind: kind.map(convert_program_kind),
            path,
            time_limit_secs,
            memory_limit_mb,
            compile_args: compile_arg,
            replace,
        })?,
        AddCommands::Bundle {
            name,
            work_dir,
            generator,
            cases,
            replace,
        } => tool::add_bundle(tool::AddBundleOptions {
            work_dir,
            name,
            generator,
            cases: cases.into_iter().map(parse_case_args).collect(),
            replace,
        })?,
        AddCommands::Task {
            name,
            work_dir,
            score,
            task_type,
            bundles,
            dependencies,
            expect_pass,
            expect_fail,
            replace,
        } => tool::add_task(tool::AddTaskOptions {
            work_dir,
            name,
            score,
            task_type: convert_task_type(task_type),
            bundles,
            dependencies,
            expect_pass,
            expect_fail,
            replace,
        })?,
        AddCommands::Validator {
            name,
            work_dir,
            time_limit_secs,
            memory_limit_mb,
            compile_arg,
            replace,
        } => tool::add_validator(tool::AddValidatorOptions {
            work_dir,
            name,
            time_limit_secs,
            memory_limit_mb,
            compile_args: compile_arg,
            replace,
        })?,
        AddCommands::Checker {
            name,
            work_dir,
            builtin,
            time_limit_secs,
            memory_limit_mb,
            compile_arg,
            replace,
        } => tool::add_checker(tool::AddCheckerOptions {
            work_dir,
            name,
            builtin,
            time_limit_secs,
            memory_limit_mb,
            compile_args: compile_arg,
            replace,
        })?,
    }
    Ok(())
}

fn parse_case_args(value: String) -> Vec<String> {
    if value.is_empty() {
        Vec::new()
    } else {
        value.split(',').map(str::to_string).collect()
    }
}

fn convert_program_kind(value: AddProgramKindArg) -> tool::AddProgramKind {
    match value {
        AddProgramKindArg::Cpp => tool::AddProgramKind::Cpp,
        AddProgramKindArg::Python => tool::AddProgramKind::Python,
        AddProgramKindArg::Command => tool::AddProgramKind::Command,
    }
}

fn convert_task_type(value: AddTaskTypeArg) -> tool::TestTaskType {
    match value {
        AddTaskTypeArg::Min => tool::TestTaskType::Min,
        AddTaskTypeArg::Sum => tool::TestTaskType::Sum,
    }
}

fn convert_judge_expectation(value: JudgeExpectationArg) -> tool::JudgeExpectation {
    match value {
        JudgeExpectationArg::Pass => tool::JudgeExpectation::Pass,
        JudgeExpectationArg::Fail => tool::JudgeExpectation::Fail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cptool::support::count_lines;

    #[test]
    fn normalizes_single_case_selector_positional() {
        assert_eq!(
            normalize_run_positionals(Some("sample[0]".to_string()), None),
            (None, Some("sample[0]".to_string()))
        );
        assert_eq!(
            normalize_run_positionals(Some("solution".to_string()), None),
            (Some("solution".to_string()), None)
        );
        assert_eq!(
            normalize_run_positionals(Some("solution".to_string()), Some("sample[0]".to_string())),
            (Some("solution".to_string()), Some("sample[0]".to_string()))
        );
    }

    #[test]
    fn positive_seconds_rejects_zero_and_non_numbers() {
        assert_eq!(args::positive_seconds("1"), Ok(1));
        assert!(args::positive_seconds("0").is_err());
        assert!(args::positive_seconds("1.5").is_err());
        assert!(args::positive_seconds("abc").is_err());
    }

    #[test]
    fn positive_f64_rejects_non_positive_and_non_finite_values() {
        assert_eq!(args::positive_f64("1"), Ok(1.0));
        assert_eq!(args::positive_f64("0.25"), Ok(0.25));
        assert!(args::positive_f64("0").is_err());
        assert!(args::positive_f64("-1").is_err());
        assert!(args::positive_f64("NaN").is_err());
        assert!(args::positive_f64("inf").is_err());
        assert!(args::positive_f64("abc").is_err());
    }

    #[test]
    fn generation_lock_timeout_maps_seconds_to_duration() {
        assert_eq!(generation_lock_timeout(None), None);
        assert_eq!(
            generation_lock_timeout(Some(3)),
            Some(Duration::from_secs(3))
        );
    }

    #[test]
    fn stress_plan_filter_prefers_positive_when_both_are_set() {
        assert_eq!(
            stress_plan_filter(false, false),
            tool::StressPlanFilter::All
        );
        assert_eq!(
            stress_plan_filter(true, false),
            tool::StressPlanFilter::PositiveOnly
        );
        assert_eq!(
            stress_plan_filter(false, true),
            tool::StressPlanFilter::NegativeOnly
        );
        assert_eq!(
            stress_plan_filter(true, true),
            tool::StressPlanFilter::PositiveOnly
        );
    }

    #[test]
    fn count_lines_handles_trailing_newlines() {
        assert_eq!(count_lines(b""), 0);
        assert_eq!(count_lines(b"one"), 1);
        assert_eq!(count_lines(b"one\n"), 1);
        assert_eq!(count_lines(b"one\ntwo"), 2);
        assert_eq!(count_lines(b"one\ntwo\n"), 2);
    }
}
