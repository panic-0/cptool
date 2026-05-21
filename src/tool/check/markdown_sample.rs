use super::{CheckReport, Problem, codes};
use std::path::{Path, PathBuf};

pub(super) fn find_sample_bundle(problem: &Problem) -> Option<&str> {
    if problem.test.bundles.contains_key("sample") {
        return Some("sample");
    }
    if problem.test.bundles.contains_key("samples") {
        return Some("samples");
    }
    None
}

pub(super) fn check_statement_sample_output(
    report: &mut CheckReport,
    work_dir: &Path,
    problem: Option<&Problem>,
    generated_sample_answer: Option<&str>,
) {
    let statement_path = work_dir.join("statement.md");
    let Ok(statement) = std::fs::read_to_string(&statement_path) else {
        return;
    };
    let blocks = markdown_sample_output_blocks(&statement);
    if blocks.is_empty() {
        return;
    }
    if blocks.len() > 1 {
        report.warning(
            codes::SAMPLE_OUTPUT_AMBIGUOUS,
            "multiple sample output code blocks were found in statement.md; skipped comparison",
            Some(statement_path),
        );
        return;
    }

    let answer = match generated_sample_answer {
        Some(answer) => answer.to_string(),
        None => {
            let Some(answer_path) = sample_answer_from_data_dir(work_dir, problem) else {
                report.warning(
                    codes::SAMPLE_ANSWER_MISSING,
                    "sample output was found in statement.md, but sample-0.ans is unavailable",
                    Some(statement_path),
                );
                return;
            };
            let Ok(answer) = std::fs::read_to_string(&answer_path) else {
                report.warning(
                    codes::SAMPLE_ANSWER_UNREADABLE,
                    "sample-0.ans exists but could not be read",
                    Some(answer_path),
                );
                return;
            };
            answer
        }
    };

    if normalize_output_block(&blocks[0]) != normalize_output_block(&answer) {
        report.error(
            codes::STATEMENT_SAMPLE_OUTPUT_MISMATCH,
            "statement.md sample output does not match sample-0.ans",
            Some(statement_path),
        );
    }
}

#[cfg(test)]
pub(super) fn sample_answer_from_data_dir(
    work_dir: &Path,
    problem: Option<&Problem>,
) -> Option<PathBuf> {
    sample_answer_path(work_dir, problem)
}

#[cfg(not(test))]
fn sample_answer_from_data_dir(work_dir: &Path, problem: Option<&Problem>) -> Option<PathBuf> {
    sample_answer_path(work_dir, problem)
}

fn sample_answer_path(work_dir: &Path, problem: Option<&Problem>) -> Option<PathBuf> {
    let bundle = problem.and_then(find_sample_bundle).unwrap_or("sample");
    let path = work_dir.join("data").join(format!("{bundle}-0.ans"));
    path.is_file().then_some(path)
}

#[cfg(test)]
pub(super) fn markdown_sample_output_blocks(markdown: &str) -> Vec<String> {
    collect_markdown_sample_output_blocks(markdown)
}

#[cfg(not(test))]
fn markdown_sample_output_blocks(markdown: &str) -> Vec<String> {
    collect_markdown_sample_output_blocks(markdown)
}

fn collect_markdown_sample_output_blocks(markdown: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut in_fence = false;
    let mut fence_marker = "";
    let mut capture = false;
    let mut current = String::new();
    let mut pending_output = false;

    for line in markdown.lines() {
        let trimmed = line.trim_start();
        let is_fence = trimmed.starts_with("```") || trimmed.starts_with("~~~");
        if is_fence {
            let marker = &trimmed[..3];
            if !in_fence {
                in_fence = true;
                fence_marker = marker;
                capture = pending_output;
                pending_output = false;
                current.clear();
            } else if marker == fence_marker {
                in_fence = false;
                if capture {
                    blocks.push(current.clone());
                }
                capture = false;
            } else if capture {
                current.push_str(line);
                current.push('\n');
            }
            continue;
        }

        if in_fence {
            if capture {
                current.push_str(line);
                current.push('\n');
            }
            continue;
        }

        if line.trim().is_empty() {
            continue;
        }
        pending_output = is_sample_output_context(line);
    }

    blocks
}

fn is_sample_output_context(line: &str) -> bool {
    let line = line
        .trim()
        .trim_start_matches('#')
        .trim()
        .to_ascii_lowercase();
    line.contains("sample output")
        || line.contains("output sample")
        || line.contains("样例输出")
        || line.contains("输出样例")
}

#[cfg(test)]
pub(super) fn normalize_output_block(value: &str) -> String {
    normalize_output_block_impl(value)
}

#[cfg(not(test))]
fn normalize_output_block(value: &str) -> String {
    normalize_output_block_impl(value)
}

fn normalize_output_block_impl(value: &str) -> String {
    let normalized = value.replace("\r\n", "\n").replace('\r', "\n");
    let lines = normalized
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n");
    lines.trim_matches('\n').to_string()
}
