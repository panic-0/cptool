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

fn parse_range_arg(arg: &str) -> anyhow::Result<Option<Vec<String>>> {
    let Some((start, end)) = parse_dot_range_arg(arg).or_else(|| parse_legacy_range_arg(arg))
    else {
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

fn parse_dot_range_arg(arg: &str) -> Option<(&str, &str)> {
    let (start, end) = arg.split_once("..")?;
    if start.is_empty() || end.is_empty() || end.contains("..") {
        return None;
    }
    Some((start, end))
}

fn parse_legacy_range_arg(arg: &str) -> Option<(&str, &str)> {
    let inner = arg.strip_prefix('{')?.strip_suffix('}')?;
    let (start, end) = inner.split_once(':')?;
    if end.contains(':') {
        return None;
    }
    Some((start, end))
}

fn legacy_expand_arg(arg: &str, case: usize, case0: usize) -> String {
    arg.replace("{case0}", &case0.to_string())
        .replace("{case}", &case.to_string())
}
