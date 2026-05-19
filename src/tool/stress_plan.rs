use super::problem::load_problem;
use super::schema::StressPlan;
use super::stress::{StressRunOptions, StressSummary, run_stress};
use anyhow::{Context, Result};
use std::path::Path;

const DEFAULT_SEED_BASE: u64 = 0xc2b2_ae3d_27d4_eb4f;
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

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
            args_by_case: args_by_case(plan),
            failure_dir,
            output_limit_bytes,
            plan_name: Some(&plan.name),
            print_progress: !summary_only,
            print_warnings: !summary_only,
        })
        .with_context(|| format!("stress plan `{}` failed", plan.name))?;
        if summary_only {
            println!("{}", summary.summary_line());
        } else {
            println!(
                "stress plan `{}` passed: {} cases",
                plan.name, summary.cases
            );
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

fn args_by_case(plan: &StressPlan) -> Vec<Vec<String>> {
    (0..plan.cases)
        .map(|case0| {
            let case = case0 + 1;
            let seed = derive_seed(plan, case0);
            plan.args
                .iter()
                .map(|arg| expand_arg(arg, case, case0, seed))
                .collect()
        })
        .collect()
}

fn expand_arg(arg: &str, case: usize, case0: usize, seed: u64) -> String {
    arg.replace("{seed}", &seed.to_string())
        .replace("{case0}", &case0.to_string())
        .replace("{case}", &case.to_string())
}

fn derive_seed(plan: &StressPlan, case0: usize) -> u64 {
    let mut state = FNV_OFFSET_BASIS ^ plan.seed_base.unwrap_or(DEFAULT_SEED_BASE);
    for byte in plan.name.as_bytes() {
        state ^= u64::from(*byte);
        state = state.wrapping_mul(FNV_PRIME);
    }
    state ^= (case0 as u64)
        .wrapping_add(1)
        .wrapping_mul(0x9e37_79b9_7f4a_7c15);
    splitmix64(state)
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
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
        };

        let args = args_by_case(&plan);

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

        assert_eq!(args_by_case(&first), args_by_case(&first));
        assert_ne!(args_by_case(&first), args_by_case(&second));
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
        }
    }
}
