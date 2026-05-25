use super::data::{format_duration, wait_for_generation_status};
use super::problem::load_problem;
use super::schema::{Problem, TestTask};
use super::stress::{StressRunOptions, StressSummary, run_stress};
use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

#[derive(Clone, Copy, Debug)]
pub struct TaskExpectOptions<'a> {
    pub work_dir: &'a Path,
    pub name: Option<&'a str>,
    pub failure_dir: Option<&'a Path>,
    pub output_limit_bytes: usize,
    pub summary_only: bool,
    pub generation_lock_timeout: Option<Duration>,
}

struct TaskExpectRunContext<'a> {
    work_dir: &'a Path,
    problem: &'a Problem,
    failure_dir: Option<&'a Path>,
    output_limit_bytes: usize,
    print_progress: bool,
    print_warnings: bool,
}

pub fn task_expect_with_options(options: TaskExpectOptions<'_>) -> Result<Vec<StressSummary>> {
    task_expect_impl(options, true)
}

pub fn task_expect_collect_with_options(
    options: TaskExpectOptions<'_>,
) -> Result<Vec<StressSummary>> {
    task_expect_impl(options, false)
}

fn task_expect_impl(options: TaskExpectOptions<'_>, emit_text: bool) -> Result<Vec<StressSummary>> {
    let TaskExpectOptions {
        work_dir,
        name,
        failure_dir,
        output_limit_bytes,
        summary_only,
        generation_lock_timeout,
    } = options;
    wait_for_generation_lock(work_dir, generation_lock_timeout)?;
    let problem = load_problem(work_dir)?;
    let tasks = select_expect_tasks(&problem, name)?;
    let mut summaries = Vec::new();

    for task in tasks {
        let context = TaskExpectRunContext {
            work_dir,
            problem: &problem,
            failure_dir,
            output_limit_bytes,
            print_progress: emit_text && !summary_only,
            print_warnings: emit_text && !summary_only,
        };
        let mut task_summaries = run_task_expect(&context, task)
            .with_context(|| format!("expect task `{}` failed", task.name))?;
        if emit_text && summary_only {
            for summary in &task_summaries {
                println!("{}", summary.summary_line());
            }
        } else if emit_text {
            println!(
                "expect task `{}` passed: {} checks",
                task.name,
                task_summaries.len()
            );
        }
        summaries.append(&mut task_summaries);
    }

    Ok(summaries)
}

fn run_task_expect(
    context: &TaskExpectRunContext<'_>,
    task: &TestTask,
) -> Result<Vec<StressSummary>> {
    let TaskExpectRunContext {
        work_dir,
        problem,
        failure_dir,
        output_limit_bytes,
        print_progress,
        print_warnings,
    } = *context;
    let cases_by_generator = task_cases_by_generator(problem, task)?;
    let mut summaries = Vec::new();
    for program in &task.pass_programs {
        for (generator, args_by_case) in &cases_by_generator {
            let against = vec![problem.solution_name.clone(), program.clone()];
            summaries.push(run_stress(StressRunOptions {
                work_dir,
                generator,
                against: &against,
                args_by_case: args_by_case.clone(),
                failure_dir,
                output_limit_bytes,
                plan_name: Some(&format!("{}:pass:{program}", task.name)),
                progress_label: "task",
                print_progress,
                print_warnings,
                expect_failure: false,
                allow_expected_failure_absent: false,
            })?);
        }
    }
    for program in &task.fail_programs {
        let mut program_summaries = Vec::new();
        for (generator, args_by_case) in &cases_by_generator {
            let against = vec![problem.solution_name.clone(), program.clone()];
            program_summaries.push(run_stress(StressRunOptions {
                work_dir,
                generator,
                against: &against,
                args_by_case: args_by_case.clone(),
                failure_dir,
                output_limit_bytes,
                plan_name: Some(&format!("{}:fail:{program}", task.name)),
                progress_label: "task",
                print_progress,
                print_warnings,
                expect_failure: true,
                allow_expected_failure_absent: true,
            })?);
        }
        if !program_summaries
            .iter()
            .any(|summary| summary.expected_failure.is_some())
        {
            anyhow::bail!(
                "fail program `{program}` passed all cases in task `{}`",
                task.name
            );
        }
        summaries.extend(program_summaries);
    }
    Ok(summaries)
}

fn task_cases_by_generator(
    problem: &Problem,
    task: &TestTask,
) -> Result<BTreeMap<String, Vec<Vec<String>>>> {
    let mut cases = BTreeMap::<String, Vec<Vec<String>>>::new();
    for bundle_name in &task.bundles {
        let bundle = problem
            .test
            .bundles
            .get(bundle_name)
            .with_context(|| format!("bundle `{bundle_name}` not found"))?;
        for case in &bundle.cases {
            cases
                .entry(case.generator_name.clone())
                .or_default()
                .push(case.args.clone());
        }
    }
    for case in &task.cases {
        cases
            .entry(case.generator_name.clone())
            .or_default()
            .push(case.args.clone());
    }
    Ok(cases)
}

fn wait_for_generation_lock(work_dir: &Path, timeout: Option<Duration>) -> Result<()> {
    let Some(timeout) = timeout else {
        return Ok(());
    };
    let data_dir = work_dir.join("data");
    if let Some(status) = wait_for_generation_status(&data_dir, timeout) {
        anyhow::bail!(
            "data generation is still in progress after waiting {}: {} (retry after current generation finishes or prewarm the selector serially)",
            format_duration(timeout),
            status.marker_path.display()
        );
    }
    Ok(())
}

fn select_expect_tasks<'a>(problem: &'a Problem, name: Option<&str>) -> Result<Vec<&'a TestTask>> {
    if let Some(name) = name {
        if let Some(task) = problem.test.tasks.iter().find(|task| task.name == name) {
            if task.has_expectations() {
                return Ok(vec![task]);
            }
            anyhow::bail!("expect task `{name}` has no pass or fail checks");
        }
        anyhow::bail!("expect task `{name}` not found");
    }
    let selected = problem
        .test
        .tasks
        .iter()
        .filter(|task| task.has_expectations())
        .collect::<Vec<_>>();
    if selected.is_empty() {
        anyhow::bail!("problem.yaml has no test.tasks with pass or fail checks");
    }
    Ok(selected)
}
