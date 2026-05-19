use super::problem::load_problem;
use super::schema::StressPlan;
use super::stress::{StressRunOptions, StressSummary, run_stress};
use anyhow::{Context, Result};
use std::path::Path;

pub fn stress_plan(
    work_dir: &Path,
    name: Option<&str>,
    failure_dir: Option<&Path>,
    output_limit_bytes: usize,
) -> Result<Vec<StressSummary>> {
    let problem = load_problem(work_dir)?;
    let plans = select_plans(&problem.stress.plans, name)?;
    let mut summaries = Vec::with_capacity(plans.len());

    for plan in plans {
        let summary = run_stress(StressRunOptions {
            work_dir,
            generator: &plan.generator,
            against: &plan.against,
            args_by_case: args_by_case(plan),
            failure_dir,
            output_limit_bytes,
            plan_name: Some(&plan.name),
        })
        .with_context(|| format!("stress plan `{}` failed", plan.name))?;
        println!(
            "stress plan `{}` passed: {} cases",
            plan.name, summary.cases
        );
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

fn args_by_case(plan: &StressPlan) -> Vec<Vec<String>> {
    (0..plan.cases).map(|_| plan.args.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_args_are_passed_literally_for_each_case() {
        let plan = StressPlan {
            name: "small".to_string(),
            generator: "gen".to_string(),
            args: vec![
                "--seed={seed}".to_string(),
                "--case={case}".to_string(),
                "--case0={case0}".to_string(),
            ],
            against: vec!["std".to_string(), "brute".to_string()],
            cases: 3,
        };

        assert_eq!(
            args_by_case(&plan),
            vec![
                vec![
                    "--seed={seed}".to_string(),
                    "--case={case}".to_string(),
                    "--case0={case0}".to_string()
                ],
                vec![
                    "--seed={seed}".to_string(),
                    "--case={case}".to_string(),
                    "--case0={case0}".to_string()
                ],
                vec![
                    "--seed={seed}".to_string(),
                    "--case={case}".to_string(),
                    "--case0={case0}".to_string()
                ],
            ]
        );
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
        }
    }
}
