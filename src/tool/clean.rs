use super::data::data_generation_status;
use super::problem::normalize_work_dir;
use anyhow::{Context, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct CleanOptions {
    pub work_dir: PathBuf,
    pub data: bool,
    pub cache: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct CleanReport {
    pub work_dir: PathBuf,
    pub data_files_removed: usize,
    pub cache_removed: bool,
    pub failures_removed: bool,
    pub paths_removed: Vec<PathBuf>,
}

impl CleanReport {
    pub fn summary_line(&self) -> String {
        format!(
            "cleaned data_files={} cache_removed={} failures_removed={}",
            self.data_files_removed, self.cache_removed, self.failures_removed
        )
    }
}

pub fn clean_package_with_options(options: CleanOptions) -> Result<CleanReport> {
    let work_dir = normalize_work_dir(&options.work_dir)?;
    let clean_data = options.data || !options.cache;
    let clean_cache = options.cache || !options.data;
    let mut report = CleanReport {
        work_dir: work_dir.clone(),
        data_files_removed: 0,
        cache_removed: false,
        failures_removed: false,
        paths_removed: Vec::new(),
    };

    if clean_data {
        clean_data_files(&work_dir, &mut report)?;
    }
    if clean_cache {
        clean_cache_dir(&work_dir, &mut report)?;
        clean_failures_dir(&work_dir, &mut report)?;
    }
    Ok(report)
}

fn clean_data_files(work_dir: &Path, report: &mut CleanReport) -> Result<()> {
    let data_dir = work_dir.join("data");
    if !data_dir.exists() {
        return Ok(());
    }
    if let Some(status) = data_generation_status(&data_dir) {
        anyhow::bail!(
            "data generation is in progress: {}; refusing to clean data files",
            status.marker_path.display()
        );
    }
    let entries = std::fs::read_dir(&data_dir)
        .with_context(|| format!("failed to read data dir {}", data_dir.display()))?;
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && is_data_file(&path) {
            paths.push(path);
        }
    }
    paths.sort();
    for path in paths {
        std::fs::remove_file(&path)
            .with_context(|| format!("failed to remove data file {}", path.display()))?;
        report.data_files_removed += 1;
        report.paths_removed.push(path);
    }
    Ok(())
}

fn clean_cache_dir(work_dir: &Path, report: &mut CleanReport) -> Result<()> {
    let cache_dir = work_dir.join(".cptool").join("cache");
    if !cache_dir.exists() {
        return Ok(());
    }
    std::fs::remove_dir_all(&cache_dir)
        .with_context(|| format!("failed to remove cache dir {}", cache_dir.display()))?;
    report.cache_removed = true;
    report.paths_removed.push(cache_dir);
    Ok(())
}

fn clean_failures_dir(work_dir: &Path, report: &mut CleanReport) -> Result<()> {
    let failures_dir = work_dir.join(".cptool").join("failures");
    if !failures_dir.exists() {
        return Ok(());
    }
    std::fs::remove_dir_all(&failures_dir)
        .with_context(|| format!("failed to remove failures dir {}", failures_dir.display()))?;
    report.failures_removed = true;
    report.paths_removed.push(failures_dir);
    Ok(())
}

fn is_data_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("in" | "ans")
    )
}
