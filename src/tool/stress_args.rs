use super::schema::StressPlan;

pub(crate) fn direct_stress_args_by_case(args: &[String], cases: usize) -> Vec<Vec<String>> {
    (0..cases).map(|_| args.to_vec()).collect()
}

pub(crate) fn legacy_stress_args_by_case(args: &[String], cases: usize) -> Vec<Vec<String>> {
    (0..cases)
        .map(|case0| {
            let case = case0 + 1;
            args.iter()
                .map(|arg| legacy_expand_arg(arg, case, case0))
                .collect()
        })
        .collect()
}

pub fn range_args(args: &[String]) -> anyhow::Result<Vec<Vec<String>>> {
    let choices = args
        .iter()
        .map(|arg| {
            if let Some(values) = parse_range_arg(arg)? {
                Ok(values)
            } else {
                Ok(vec![arg.clone()])
            }
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let mut expanded = vec![Vec::new()];
    for values in choices {
        let mut next = Vec::with_capacity(expanded.len().saturating_mul(values.len()));
        for prefix in &expanded {
            for value in &values {
                let mut args = prefix.clone();
                args.push(value.clone());
                next.push(args);
            }
        }
        expanded = next;
    }
    Ok(expanded)
}

pub(crate) fn plan_args_by_case(plan: &StressPlan) -> Vec<Vec<String>> {
    expand_args_by_case(&plan.args, plan.cases)
}

fn parse_range_arg(arg: &str) -> anyhow::Result<Option<Vec<String>>> {
    let Some(inner) = arg
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
    else {
        return Ok(None);
    };
    let Some((start, end)) = inner.split_once(':') else {
        return Ok(None);
    };
    let start = start
        .parse::<i64>()
        .map_err(|_| anyhow::anyhow!("invalid range start in generator arg `{arg}`"))?;
    let end = end
        .parse::<i64>()
        .map_err(|_| anyhow::anyhow!("invalid range end in generator arg `{arg}`"))?;
    if start > end {
        anyhow::bail!("range generator arg `{arg}` has start greater than end");
    }
    Ok(Some((start..=end).map(|value| value.to_string()).collect()))
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
    let _ = (case, case0);
    arg.to_string()
}

fn legacy_expand_arg(arg: &str, case: usize, case0: usize) -> String {
    arg.replace("{case0}", &case0.to_string())
        .replace("{case}", &case.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_stress_args_repeat_literal_args() {
        let args = direct_stress_args_by_case(
            &["--seed=literal".to_string(), "--mode=fixed".to_string()],
            3,
        );

        assert_eq!(args.len(), 3);
        assert_eq!(args[0][0], "--seed=literal");
        assert_eq!(args[0][1], "--mode=fixed");
        assert_eq!(args[1][0], "--seed=literal");
        assert_eq!(args[1][1], "--mode=fixed");
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
