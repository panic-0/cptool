use super::schema::StressPlan;

pub(crate) fn direct_stress_args_by_case(args: &[String], cases: usize) -> Vec<Vec<String>> {
    expand_args_by_case(args, cases)
}

pub(crate) fn plan_args_by_case(plan: &StressPlan) -> Vec<Vec<String>> {
    expand_args_by_case(&plan.args, plan.cases)
}

fn expand_args_by_case(args: &[String], cases: usize) -> Vec<Vec<String>> {
    (0..cases)
        .map(|case0| {
            let case = case0 + 1;
            args.iter()
                .map(|arg| expand_arg(arg, case, case0))
                .collect()
        })
        .collect()
}

fn expand_arg(arg: &str, case: usize, case0: usize) -> String {
    arg.replace("{case0}", &case0.to_string())
        .replace("{case}", &case.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_stress_args_expand_case_placeholders() {
        let args = direct_stress_args_by_case(
            &[
                "--case={case}".to_string(),
                "--case0={case0}".to_string(),
                "--literal=case".to_string(),
            ],
            3,
        );

        assert_eq!(args.len(), 3);
        assert_eq!(args[0][0], "--case=1");
        assert_eq!(args[0][1], "--case0=0");
        assert_eq!(args[1][0], "--case=2");
        assert_eq!(args[1][1], "--case0=1");
        assert_eq!(args[2][2], "--literal=case");
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
