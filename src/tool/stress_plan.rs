use super::problem::load_problem;
use super::schema::{StressPlan, StressPlanExpectation};
use super::stress::{StressRunOptions, StressSummary, run_stress};
use super::stress_args::plan_args_by_case;
use anyhow::{Context, Result};
use std::path::Path;

#[derive(Clone, Copy, Debug)]
pub struct StressPlanOptions<'a> {
    pub work_dir: &'a Path,
    pub name: Option<&'a str>,
    pub failure_dir: Option<&'a Path>,
    pub output_limit_bytes: usize,
    pub summary_only: bool,
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
    } = options;
    let problem = load_problem(work_dir)?;
    let plans = select_plans(&problem.stress.plans, name)?;
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

fn select_plans<'a>(plans: &'a [StressPlan], name: Option<&str>) -> Result<Vec<&'a StressPlan>> {
    if plans.is_empty() {
        anyhow::bail!("problem.yaml has no stress.plans");
    }
    if let Some(name) = name {
        let plan = plans
            .iter()
            .find(|plan| plan.name == name)
            .with_context(|| format!("stress plan `{name}` not found"))?;
        return Ok(vec![plan]);
    }
    Ok(plans.iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_args_expand_seed_and_case_placeholders() {
        let plan = StressPlan {
            name: "small".to_string(),
            generator: "gen".to_string(),
            args: vec![
                "--seed={seed}".to_string(),
                "--case={case}".to_string(),
                "--case0={case0}".to_string(),
                "--literal=case".to_string(),
            ],
            against: vec!["std".to_string(), "brute".to_string()],
            cases: 3,
            seed_base: Some(42),
            expect: StressPlanExpectation::Pass,
        };

        let args = plan_args_by_case(&plan);

        assert_eq!(args.len(), 3);
        assert_eq!(args[0][1], "--case=1");
        assert_eq!(args[0][2], "--case0=0");
        assert_eq!(args[1][1], "--case=2");
        assert_eq!(args[1][2], "--case0=1");
        assert_eq!(args[2][3], "--literal=case");
        assert_ne!(args[0][0], args[1][0]);
        assert!(
            args[0][0]
                .strip_prefix("--seed=")
                .unwrap()
                .parse::<u64>()
                .is_ok()
        );
    }

    #[test]
    fn seed_base_changes_deterministic_seeds() {
        let mut first = plan("small");
        first.args = vec!["{seed}".to_string()];
        first.seed_base = Some(1);
        let mut second = first.clone();
        second.seed_base = Some(2);

        assert_eq!(plan_args_by_case(&first), plan_args_by_case(&first));
        assert_ne!(plan_args_by_case(&first), plan_args_by_case(&second));
    }

    #[test]
    fn selects_all_or_named_plan() {
        let plans = vec![plan("small"), plan("large")];

        assert_eq!(select_plans(&plans, None).unwrap().len(), 2);
        assert_eq!(
            select_plans(&plans, Some("large")).unwrap()[0].name,
            "large"
        );
        assert!(select_plans(&plans, Some("missing")).is_err());
    }

    fn plan(name: &str) -> StressPlan {
        StressPlan {
            name: name.to_string(),
            generator: "gen".to_string(),
            args: Vec::new(),
            against: vec!["std".to_string(), "brute".to_string()],
            cases: 1,
            seed_base: None,
            expect: StressPlanExpectation::Pass,
        }
    }
}
