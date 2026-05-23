use super::data::{format_duration, wait_for_generation_status};
use super::problem::load_problem;
use super::schema::{StressPlan, StressPlanExpectation};
use super::stress::{StressRunOptions, StressSummary, run_stress};
use super::stress_args::plan_args_by_case;
use anyhow::{Context, Result};
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

pub fn stress_plan(
    work_dir: &Path,
    name: Option<&str>,
    failure_dir: Option<&Path>,
    output_limit_bytes: usize,
) -> Result<Vec<StressSummary>> {
    stress_plan_with_options(StressPlanOptions {
        work_dir,
        name,
        failure_dir,
        output_limit_bytes,
        summary_only: false,
        filter: StressPlanFilter::All,
        generation_lock_timeout: None,
    })
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
    let plans = select_plans(&problem.stress.plans, name, filter)?;
    let mut summaries = Vec::with_capacity(plans.len());

    for plan in plans {
        let summary = run_stress(StressRunOptions {
            work_dir,
            generator: &plan.generator,
            against: &plan.against,
            args_by_case: plan_args_by_case(plan),
            failure_dir,
            output_limit_bytes,
            plan_name: Some(&plan.name),
            print_progress: emit_text && !summary_only,
            print_warnings: emit_text && !summary_only,
            expect_failure: plan.expect == StressPlanExpectation::Fail,
        })
        .with_context(|| format!("stress plan `{}` failed", plan.name))?;
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

    Ok(summaries)
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
    fn plan_args_expand_case_placeholders() {
        let plan = StressPlan {
            name: "small".to_string(),
            generator: "gen".to_string(),
            args: vec![
                "--case={case}".to_string(),
                "--case0={case0}".to_string(),
                "--literal=case".to_string(),
            ],
            against: vec!["std".to_string(), "brute".to_string()],
            cases: 3,
            expect: StressPlanExpectation::Pass,
        };

        let args = plan_args_by_case(&plan);

        assert_eq!(args.len(), 3);
        assert_eq!(args[0][0], "--case=1");
        assert_eq!(args[0][1], "--case0=0");
        assert_eq!(args[1][0], "--case=2");
        assert_eq!(args[1][1], "--case0=1");
        assert_eq!(args[2][2], "--literal=case");
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
