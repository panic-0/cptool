use super::schema::StressPlan;

const DEFAULT_SEED_BASE: u64 = 0xc2b2_ae3d_27d4_eb4f;
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

pub(crate) fn direct_stress_args_by_case(args: &[String], cases: usize) -> Vec<Vec<String>> {
    expand_args_by_case("stress", None, args, cases)
}

pub(crate) fn plan_args_by_case(plan: &StressPlan) -> Vec<Vec<String>> {
    expand_args_by_case(&plan.name, plan.seed_base, &plan.args, plan.cases)
}

fn expand_args_by_case(
    seed_name: &str,
    seed_base: Option<u64>,
    args: &[String],
    cases: usize,
) -> Vec<Vec<String>> {
    (0..cases)
        .map(|case0| {
            let case = case0 + 1;
            let seed = derive_seed(seed_name, seed_base, case0);
            args.iter()
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

fn derive_seed(seed_name: &str, seed_base: Option<u64>, case0: usize) -> u64 {
    let mut state = FNV_OFFSET_BASIS ^ seed_base.unwrap_or(DEFAULT_SEED_BASE);
    for byte in seed_name.as_bytes() {
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
    fn direct_stress_args_expand_seed_and_case_placeholders() {
        let args = direct_stress_args_by_case(
            &[
                "--seed={seed}".to_string(),
                "--case={case}".to_string(),
                "--case0={case0}".to_string(),
                "--literal=case".to_string(),
            ],
            3,
        );

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
    fn fixed_direct_stress_args_remain_literal_for_each_case() {
        let args = vec!["10".to_string(), "case".to_string()];

        assert_eq!(
            direct_stress_args_by_case(&args, 2),
            vec![args.clone(), args]
        );
    }
}
