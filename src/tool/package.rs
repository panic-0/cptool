use anyhow::Result;
use std::path::{Path, PathBuf};

const TESTLIB_H: &str = include_str!("../../third_party/testlib/testlib.h");
pub(crate) const DEFAULT_PROGRAM_CPP: &str =
    include_str!("../../assets/templates/default_program.cpp");
const DEFAULT_GENERATOR_CPP: &str = r#"#include "testlib.h"

int main(int argc, char *argv[]) {
    registerGen(argc, argv, 1);
    return 0;
}
"#;
const DEFAULT_VALIDATOR_CPP: &str = r#"#include "testlib.h"

int main(int argc, char *argv[]) {
    registerValidation(argc, argv);
    inf.readEof();
    return 0;
}
"#;
const DEFAULT_CHECKER_CPP: &str = concat!(
    "// Copied from testlib checkers/wcmp.cpp\n",
    include_str!("../../third_party/testlib/checkers/wcmp.cpp")
);

pub fn init_package(root: &Path, id: &str) -> Result<PathBuf> {
    let slug = slugify(id)?;
    let problem_dir = problems_dir_for_root(root).join(slug);
    if problem_dir.exists() {
        anyhow::bail!("problem package already exists: {}", problem_dir.display());
    }

    std::fs::create_dir_all(problem_dir.join("src"))?;
    std::fs::create_dir_all(problem_dir.join("data"))?;
    std::fs::create_dir_all(problem_dir.join("fixtures").join("input"))?;
    std::fs::create_dir_all(problem_dir.join("fixtures").join("validator").join("pass"))?;
    std::fs::create_dir_all(problem_dir.join("fixtures").join("validator").join("fail"))?;
    std::fs::create_dir_all(problem_dir.join("fixtures").join("checker").join("pass"))?;
    std::fs::create_dir_all(problem_dir.join("fixtures").join("checker").join("fail"))?;
    std::fs::create_dir_all(problem_dir.join(".cptool").join("failures"))?;
    std::fs::write(problem_dir.join("statement.md"), "# 题面\n\n")?;
    std::fs::write(problem_dir.join("editorial.md"), "# 题解\n\n")?;
    std::fs::write(
        problem_dir.join(".gitignore"),
        ".cptool/\ndata/\nexport/\noutput/\ntmp/\n*.exe\n*.tmp\n",
    )?;
    std::fs::write(problem_dir.join("src").join("std.cpp"), DEFAULT_PROGRAM_CPP)?;
    std::fs::write(
        problem_dir.join("src").join("brute.cpp"),
        DEFAULT_PROGRAM_CPP,
    )?;
    std::fs::write(
        problem_dir.join("src").join("gen.cpp"),
        DEFAULT_GENERATOR_CPP,
    )?;
    std::fs::write(
        problem_dir.join("src").join("val.cpp"),
        DEFAULT_VALIDATOR_CPP,
    )?;
    std::fs::write(problem_dir.join("src").join("chk.cpp"), DEFAULT_CHECKER_CPP)?;
    std::fs::write(problem_dir.join("src").join("testlib.h"), TESTLIB_H)?;
    let yaml_name = serde_yml::to_string(id)?.trim_end().to_string();
    std::fs::write(
        problem_dir.join("problem.yaml"),
        format!(
            "name: {yaml_name}\ntime_limit_secs: 3.0\nmemory_limit_mb: 512.0\ncpp_compile_args: [-O2, -std=c++20]\nprograms:\n  gen:\n    info: !cpp\n      path: ./src/gen.cpp\n  std:\n    info: !cpp\n      path: ./src/std.cpp\n  brute:\n    info: !cpp\n      path: ./src/brute.cpp\n  val:\n    info: !cpp\n      path: ./src/val.cpp\n  chk:\n    info: !cpp\n      path: ./src/chk.cpp\nsolution: std\nvalidator: val\nchecker: chk\ngenerator: gen\ntest:\n  type: min\n  bundles:\n    sample:\n      cases:\n      - []\n  tasks:\n  - name: sample\n    score: 100.0\n    bundles: [sample]\n",
        ),
    )?;
    Ok(problem_dir)
}

fn problems_dir_for_root(root: &Path) -> PathBuf {
    if root
        .file_name()
        .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("problems"))
    {
        root.to_path_buf()
    } else {
        root.join("problems")
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::temp_test_dir;

    #[test]
    fn init_package_uses_root_as_workspace_by_default() {
        let root = temp_test_dir("cptool-init-root-default");

        let problem_dir = init_package(&root, "Default Root").unwrap();

        assert_eq!(problem_dir, root.join("problems").join("default-root"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn init_package_accepts_existing_problems_dir_as_root() {
        let root = temp_test_dir("cptool-init-root-problems");
        let problems_dir = root.join("problems");

        let problem_dir = init_package(&problems_dir, "Agent 45").unwrap();

        assert_eq!(problem_dir, problems_dir.join("agent-45"));
        assert!(!problems_dir.join("problems").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}
