use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let package_version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "unknown".into());
    let commit =
        git_output(["rev-parse", "--short=12", "HEAD"]).unwrap_or_else(|| "unknown".into());
    let dirty = git_dirty();
    let dirty_suffix = if dirty { "-dirty" } else { "" };

    println!("cargo:rustc-env=CPTOOL_VERSION={package_version} (commit {commit}{dirty_suffix})");

    emit_git_rerun_hints();
}

fn git_output<const N: usize>(args: [&str; N]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

fn git_dirty() -> bool {
    git_status(["diff", "--quiet", "--ignore-submodules", "--"]).is_some_and(|clean| !clean)
        || git_status(["diff", "--cached", "--quiet", "--ignore-submodules", "--"])
            .is_some_and(|clean| !clean)
}

fn git_status<const N: usize>(args: [&str; N]) -> Option<bool> {
    let status = Command::new("git").args(args).status().ok()?;
    Some(status.success())
}

fn emit_git_rerun_hints() {
    println!("cargo:rerun-if-changed=Cargo.toml");

    let Some(git_dir) = git_dir() else {
        return;
    };
    println!("cargo:rerun-if-changed={}", git_dir.join("HEAD").display());
    println!("cargo:rerun-if-changed={}", git_dir.join("index").display());

    let Ok(head) = std::fs::read_to_string(git_dir.join("HEAD")) else {
        return;
    };
    let Some(ref_name) = head.trim().strip_prefix("ref: ") else {
        return;
    };
    println!(
        "cargo:rerun-if-changed={}",
        git_dir.join(ref_name).display()
    );
}

fn git_dir() -> Option<PathBuf> {
    let path = Path::new(".git");
    if path.is_dir() {
        return Some(path.to_path_buf());
    }

    let content = std::fs::read_to_string(path).ok()?;
    let gitdir = content.trim().strip_prefix("gitdir: ")?;
    Some(PathBuf::from(gitdir))
}
