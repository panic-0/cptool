use super::data::{format_duration, wait_for_generation_status};
use super::problem::load_problem;
use super::schema::{Problem, StressPlan, StressPlanExpectation, TestTask};
use super::stress::{StressRunOptions, StressSummary, run_stress};
use super::stress_args::plan_args_by_case;
use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

#[derive(Clone, Copy, Debug)]
pub struct StressPlanOptions<'a> {
    pub work_dir: &'a Path,
    pub name: Option<&'a str>,
    pub failure_dir: Option<&'a Path>,
    pub output_limit_bytes: usize,
    pub summary_only: bool,
    pub filter: StressPlanFilter,
    pub generation_lock_timeout: Option<Duration>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StressPlanFilter {
    All,
    PositiveOnly,
    NegativeOnly,
}

struct TaskExpectRunContext<'a> {
    work_dir: &'a Path,
    problem: &'a Problem,
    failure_dir: Option<&'a Path>,
    output_limit_bytes: usize,
    print_progress: bool,
    print_warnings: bool,
    filter: StressPlanFilter,
}

pub fn stress_plan_with_options(options: StressPlanOptions<'_>) -> Result<Vec<StressSummary>> {
    stress_plan_impl(options, true)
}

pub fn stress_plan_collect_with_options(
    options: StressPlanOptions<'_>,
) -> Result<Vec<StressSummary>> {
    stress_plan_impl(options, false)
}

fn stress_plan_impl(options: StressPlanOptions<'_>, emit_text: bool) -> Result<Vec<StressSummary>> {
    let StressPlanOptions {
        work_dir,
        name,
        failure_dir,
        output_limit_bytes,
        summary_only,
        filter,
        generation_lock_timeout,
    } = options;
    wait_for_generation_lock(work_dir, generation_lock_timeout)?;
    let problem = load_problem(work_dir)?;
    let tasks = select_expect_tasks(&problem, name, filter)?;
    let mut summaries = Vec::new();

    for task in tasks {
        let context = TaskExpectRunContext {
            work_dir,
            problem: &problem,
            failure_dir,
            output_limit_bytes,
            print_progress: emit_text && !summary_only,
            print_warnings: emit_text && !summary_only,
            filter,
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

    if summaries.is_empty() {
        let plans = select_plans(&problem.stress.plans, name, filter)?;
        for plan in plans {
            let summary = run_legacy_plan(
                work_dir,
                plan,
                failure_dir,
                output_limit_bytes,
                emit_text && !summary_only,
                emit_text && !summary_only,
            )?;
            if emit_text {
                if summary_only {
                    println!("{}", summary.summary_line());
                } else {
                    println!(
                        "stress plan `{}` passed: {} cases",
                        plan.name, summary.cases
                    );
                }
            }
            summaries.push(summary);
        }
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
        filter,
    } = *context;
    let cases_by_generator = task_cases_by_generator(problem, task)?;
    let mut summaries = Vec::new();
    if filter != StressPlanFilter::NegativeOnly {
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
                    print_progress,
                    print_warnings,
                    expect_failure: false,
                    allow_expected_failure_absent: false,
                })?);
            }
        }
    }
    if filter != StressPlanFilter::PositiveOnly {
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
    Ok(cases)
}

fn run_legacy_plan(
    work_dir: &Path,
    plan: &StressPlan,
    failure_dir: Option<&Path>,
    output_limit_bytes: usize,
    print_progress: bool,
    print_warnings: bool,
) -> Result<StressSummary> {
    run_stress(StressRunOptions {
        work_dir,
        generator: &plan.generator,
        against: &plan.against,
        args_by_case: plan_args_by_case(plan),
        failure_dir,
        output_limit_bytes,
        plan_name: Some(&plan.name),
        print_progress,
        print_warnings,
        expect_failure: plan.expect == StressPlanExpectation::Fail,
        allow_expected_failure_absent: false,
    })
    .with_context(|| format!("stress plan `{}` failed", plan.name))
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

fn select_expect_tasks<'a>(
    problem: &'a Problem,
    name: Option<&str>,
    filter: StressPlanFilter,
) -> Result<Vec<&'a TestTask>> {
    let matches = |task: &&TestTask| match filter {
        StressPlanFilter::All => task.has_expectations(),
        StressPlanFilter::PositiveOnly => !task.pass_programs.is_empty(),
        StressPlanFilter::NegativeOnly => !task.fail_programs.is_empty(),
    };
    if let Some(name) = name {
        if let Some(task) = problem.test.tasks.iter().find(|task| task.name == name) {
            if matches(&task) {
                return Ok(vec![task]);
            }
            anyhow::bail!("expect task `{name}` does not match selected expectation filter");
        }
        return Ok(Vec::new());
    }
    Ok(problem.test.tasks.iter().filter(matches).collect())
}

fn select_plans<'a>(
    plans: &'a [StressPlan],
    name: Option<&str>,
    filter: StressPlanFilter,
) -> Result<Vec<&'a StressPlan>> {
    if plans.is_empty() {
        anyhow::bail!("problem.yaml has no stress.plans");
    }
    if let Some(name) = name {
        let plan = plans
            .iter()
            .find(|plan| plan.name == name)
            .with_context(|| format!("stress plan `{name}` not found"))?;
        if !plan_matches_filter(plan, filter) {
            anyhow::bail!("stress plan `{name}` does not match selected expectation filter");
        }
        return Ok(vec![plan]);
    }
    let selected = plans
        .iter()
        .filter(|plan| plan_matches_filter(plan, filter))
        .collect::<Vec<_>>();
    if selected.is_empty() {
        anyhow::bail!("no stress.plans matched selected expectation filter");
    }
    Ok(selected)
}

fn plan_matches_filter(plan: &StressPlan, filter: StressPlanFilter) -> bool {
    match filter {
        StressPlanFilter::All => true,
        StressPlanFilter::PositiveOnly => plan.expect == StressPlanExpectation::Pass,
        StressPlanFilter::NegativeOnly => plan.expect == StressPlanExpectation::Fail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_args_repeat_literal_args() {
        let plan = StressPlan {
            name: "small".to_string(),
            generator: "gen".to_string(),
            args: vec!["--seed=literal".to_string(), "--mode=fixed".to_string()],
            against: vec!["std".to_string(), "brute".to_string()],
            cases: 3,
            expect: StressPlanExpectation::Pass,
        };

        let args = plan_args_by_case(&plan);

        assert_eq!(args.len(), 3);
        assert_eq!(args[0][0], "--seed=literal");
        assert_eq!(args[0][1], "--mode=fixed");
        assert_eq!(args[1][0], "--seed=literal");
        assert_eq!(args[1][1], "--mode=fixed");
    }

    #[test]
    fn selects_all_or_named_plan() {
        let plans = vec![plan("small"), plan("large")];

        assert_eq!(
            select_plans(&plans, None, StressPlanFilter::All)
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            select_plans(&plans, Some("large"), StressPlanFilter::All).unwrap()[0].name,
            "large"
        );
        assert!(select_plans(&plans, Some("missing"), StressPlanFilter::All).is_err());
    }

    #[test]
    fn filters_positive_and_negative_plans() {
        let mut negative = plan("negative");
        negative.expect = StressPlanExpectation::Fail;
        let plans = vec![plan("positive"), negative];

        assert_eq!(
            select_plans(&plans, None, StressPlanFilter::PositiveOnly)
                .unwrap()
                .iter()
                .map(|plan| plan.name.as_str())
                .collect::<Vec<_>>(),
            vec!["positive"]
        );
        assert_eq!(
            select_plans(&plans, None, StressPlanFilter::NegativeOnly)
                .unwrap()
                .iter()
                .map(|plan| plan.name.as_str())
                .collect::<Vec<_>>(),
            vec!["negative"]
        );
    }

    fn plan(name: &str) -> StressPlan {
        StressPlan {
            name: name.to_string(),
            generator: "gen".to_string(),
            args: Vec::new(),
            against: vec!["std".to_string(), "brute".to_string()],
            cases: 1,
            expect: StressPlanExpectation::Pass,
        }
    }
}
