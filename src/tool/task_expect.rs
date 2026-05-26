use super::data::{format_duration, wait_for_generation_status};
use super::problem::load_problem;
use super::schema::{Problem, TestTask, TestTaskType};
use super::stress::{StressRunOptions, StressSummary, run_stress};
use anyhow::{Context, Result};
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

struct TaskCaseGroup {
    generator_name: String,
    args_by_case: Vec<Vec<String>>,
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
    let case_groups = task_case_groups(problem, task)?;
    if task
        .task_type
        .is_some_and(|task_type| task_type != TestTaskType::Min)
    {
        anyhow::bail!(
            "expect task `{}` has type `sum`; test expect tasks must use `min`",
            task.name
        );
    }
    let mut summaries = Vec::new();
    for program in &task.pass_programs {
        for group in &case_groups {
            let against = vec![problem.solution_name.clone(), program.clone()];
            summaries.push(run_stress(StressRunOptions {
                work_dir,
                generator: &group.generator_name,
                against: &against,
                args_by_case: group.args_by_case.clone(),
                failure_dir,
                output_limit_bytes,
                check_name: Some(&format!("{}:pass:{program}", task.name)),
                progress_label: "task",
                print_progress,
                print_warnings,
                expect_failure: false,
                allow_expected_failure_absent: false,
                stop_after_expected_failure: false,
            })?);
        }
    }
    for program in &task.fail_programs {
        let mut program_summaries = Vec::new();
        let mut observed_expected_failure = false;
        for group in &case_groups {
            let against = vec![problem.solution_name.clone(), program.clone()];
            let summary = run_stress(StressRunOptions {
                work_dir,
                generator: &group.generator_name,
                against: &against,
                args_by_case: group.args_by_case.clone(),
                failure_dir,
                output_limit_bytes,
                check_name: Some(&format!("{}:fail:{program}", task.name)),
                progress_label: "task",
                print_progress,
                print_warnings,
                expect_failure: true,
                allow_expected_failure_absent: true,
                stop_after_expected_failure: true,
            })?;
            observed_expected_failure = summary.expected_failure.is_some();
            program_summaries.push(summary);
            if observed_expected_failure {
                break;
            }
        }
        if !observed_expected_failure {
            anyhow::bail!(
                "fail program `{program}` passed all cases in task `{}`",
                task.name
            );
        }
        summaries.extend(program_summaries);
    }
    Ok(summaries)
}

fn task_case_groups(problem: &Problem, task: &TestTask) -> Result<Vec<TaskCaseGroup>> {
    let mut groups = Vec::<TaskCaseGroup>::new();
    for bundle_name in &task.bundles {
        let bundle = problem
            .test
            .bundles
            .get(bundle_name)
            .with_context(|| format!("bundle `{bundle_name}` not found"))?;
        for case in &bundle.cases {
            push_case_group(&mut groups, &case.generator_name, case.expanded_args());
        }
    }
    for case in &task.cases {
        push_case_group(&mut groups, &case.generator_name, case.expanded_args());
    }
    Ok(groups)
}

fn push_case_group(groups: &mut Vec<TaskCaseGroup>, generator_name: &str, args: Vec<Vec<String>>) {
    if args.is_empty() {
        return;
    }
    if let Some(last) = groups.last_mut()
        && last.generator_name == generator_name
    {
        last.args_by_case.extend(args);
        return;
    }
    groups.push(TaskCaseGroup {
        generator_name: generator_name.to_string(),
        args_by_case: args,
    });
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
