use super::{CheckReport, Problem, StressPlanExpectation};
use std::path::Path;

const SERVICE_NOISE: &[&str] = &[
    "服务侧噪音",
    "服务端噪音",
    "调度异常",
    "会话中断",
    "网络中断",
    "rate limit",
    "server-side",
];

pub(super) fn check_package_text_audit(report: &mut CheckReport, work_dir: &Path) {
    check_markdown_placeholders(report, work_dir);
    check_double_nested_problem_dir(report, work_dir);
    check_report_service_noise_and_failure_refs(report, work_dir);
}

pub(super) fn check_report_stress_plan_classification(
    report: &mut CheckReport,
    work_dir: &Path,
    problem: &Problem,
) {
    let negative_plan_names = problem
        .stress
        .plans
        .iter()
        .filter(|plan| plan.expect == StressPlanExpectation::Fail)
        .map(|plan| plan.name.as_str())
        .collect::<Vec<_>>();
    if negative_plan_names.is_empty() {
        return;
    }

    let path = work_dir.join("quality_report.md");
    let Ok(text) = read_text_lossy(&path) else {
        return;
    };

    for name in negative_plan_names {
        if text
            .lines()
            .any(|line| line.contains("正向") && line.contains(name))
        {
            report.warning(
                "negative_plan_counted_as_positive",
                format!("expect: fail plan appears in positive coverage text: {name}"),
                Some(path),
            );
            return;
        }
    }
}

fn check_markdown_placeholders(report: &mut CheckReport, work_dir: &Path) {
    for relative in ["statement.md", "editorial.md"] {
        let path = work_dir.join(relative);
        let Ok(text) = read_text_lossy(&path) else {
            continue;
        };
        if let Some(pattern) = placeholder_pattern(&text) {
            report.warning(
                "placeholder_text",
                format!("placeholder pattern matched: {pattern}"),
                Some(path),
            );
        }
    }
}

fn placeholder_pattern(text: &str) -> Option<&'static str> {
    if contains_ascii_word_ci(text, "todo") {
        return Some(r"\bTODO\b");
    }
    if text
        .lines()
        .any(|line| heading_starts_with(line, "statement"))
    {
        return Some(r"#\s*Statement\b");
    }
    if text
        .lines()
        .any(|line| heading_starts_with(line, "editorial"))
    {
        return Some(r"#\s*Editorial\b");
    }
    if text.lines().any(|line| exact_heading(line, "题面")) {
        return Some(r"#\s*题面\s*$");
    }
    if text.lines().any(|line| exact_heading(line, "题解")) {
        return Some(r"#\s*题解\s*$");
    }
    if text.contains("题面占位") {
        return Some("题面占位");
    }
    if text.contains("题解占位") {
        return Some("题解占位");
    }
    if text.contains("待补充") {
        return Some("待补充");
    }
    None
}

fn contains_ascii_word_ci(text: &str, word: &str) -> bool {
    text.as_bytes()
        .windows(word.len())
        .enumerate()
        .any(|(index, window)| {
            window.eq_ignore_ascii_case(word.as_bytes())
                && !is_ascii_word_byte(text.as_bytes().get(index.wrapping_sub(1)).copied())
                && !is_ascii_word_byte(text.as_bytes().get(index + word.len()).copied())
        })
}

fn is_ascii_word_byte(byte: Option<u8>) -> bool {
    byte.is_some_and(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn heading_starts_with(line: &str, prefix: &str) -> bool {
    let Some(heading) = markdown_heading_text(line) else {
        return false;
    };
    let lower = heading.to_ascii_lowercase();
    lower == prefix
        || lower.strip_prefix(prefix).is_some_and(|suffix| {
            !suffix.starts_with(|ch: char| ch.is_ascii_alphanumeric() || ch == '_')
        })
}

fn exact_heading(line: &str, expected: &str) -> bool {
    markdown_heading_text(line).is_some_and(|heading| heading == expected)
}

fn markdown_heading_text(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix('#')?;
    Some(rest.trim())
}

fn check_double_nested_problem_dir(report: &mut CheckReport, work_dir: &Path) {
    let Some(name) = work_dir.file_name() else {
        return;
    };
    let nested = work_dir.join(name);
    if nested.is_dir()
        && (nested.join("problem.yaml").exists() || nested.join("problem.md").exists())
    {
        report.warning(
            "double_nested_problem_dir",
            "nested directory looks like a duplicated problem root",
            Some(nested),
        );
    }
}

fn check_report_service_noise_and_failure_refs(report: &mut CheckReport, work_dir: &Path) {
    for relative in ["quality_report.md", "process_issues.md"] {
        let path = work_dir.join(relative);
        let Ok(text) = read_text_lossy(&path) else {
            continue;
        };
        let lowered = text.to_lowercase();
        if let Some(token) = SERVICE_NOISE
            .iter()
            .find(|token| lowered.contains(&token.to_lowercase()))
        {
            report.warning(
                "service_side_noise",
                format!("service-side noise token should not be a package issue: {token}"),
                Some(path.clone()),
            );
        }
        for reference in failure_references(&text) {
            if !work_dir.join(&reference).exists() {
                report.warning(
                    "missing_failure_reference",
                    format!("referenced failure artifact does not exist: {reference}"),
                    Some(path.clone()),
                );
            }
        }
    }
}

fn failure_references(text: &str) -> Vec<String> {
    let mut references = Vec::new();
    let mut start = 0;
    while let Some(offset) = text[start..].find("tests/failures/") {
        let begin = start + offset;
        let end = text[begin..]
            .char_indices()
            .find_map(|(index, ch)| (!is_failure_ref_char(ch)).then_some(begin + index))
            .unwrap_or(text.len());
        references.push(
            text[begin..end]
                .trim_end_matches(['.', ',', ')', ';', ']'])
                .to_string(),
        );
        start = end;
    }
    references
}

fn is_failure_ref_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | '-' | '/')
}

fn read_text_lossy(path: &Path) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}
