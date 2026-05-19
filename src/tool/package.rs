use anyhow::Result;
use std::path::{Path, PathBuf};
pub fn init_package(root: &Path, id: &str) -> Result<PathBuf> {
    let slug = slugify(id)?;
    let problem_dir = root.join("problems").join(slug);
    if problem_dir.exists() {
        anyhow::bail!("problem package already exists: {}", problem_dir.display());
    }

    std::fs::create_dir_all(problem_dir.join("src"))?;
    std::fs::create_dir_all(problem_dir.join("data"))?;
    std::fs::create_dir_all(problem_dir.join("tests").join("failures"))?;
    std::fs::write(problem_dir.join("statement.md"), "# 题面\n\n")?;
    std::fs::write(problem_dir.join("editorial.md"), "# 题解\n\n")?;
    std::fs::write(
        problem_dir.join(".gitignore"),
        ".cptool/\ndata/\nexport/\noutput/\ntmp/\ntests/failures/\n*.exe\n*.tmp\n",
    )?;
    std::fs::write(problem_dir.join("src").join("std.cpp"), "")?;
    std::fs::write(problem_dir.join("src").join("brute.cpp"), "")?;
    std::fs::write(problem_dir.join("src").join("gen.cpp"), "")?;
    std::fs::write(
        problem_dir.join("problem.yaml"),
        format!(
            "name: {id}\nprograms:\n  gen:\n    info: !cpp\n      path: ./src/gen.cpp\n    time_limit_secs: 1.0\n    memory_limit_mb: 512.0\n  std:\n    info: !cpp\n      path: ./src/std.cpp\n    time_limit_secs: 1.0\n    memory_limit_mb: 512.0\n  brute:\n    info: !cpp\n      path: ./src/brute.cpp\n    time_limit_secs: 1.0\n    memory_limit_mb: 512.0\nsolution: std\ntest:\n  bundles:\n    sample:\n      cases:\n      - generator: gen\n        args: []\n  tasks:\n  - name: sample\n    score: 100.0\n    type: min\n    bundles: [sample]\n",
        ),
    )?;
    Ok(problem_dir)
}

pub fn slugify(value: &str) -> Result<String> {
    let mut slug = String::new();
    let mut last_dash = false;
    for ch in value.trim().chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            slug.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if ch == '-' || !last_dash {
            slug.push('-');
            last_dash = true;
        }
    }
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        anyhow::bail!("problem id cannot be empty");
    }
    Ok(slug)
}
