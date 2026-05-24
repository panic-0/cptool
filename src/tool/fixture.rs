use super::judge::JudgeExpectation;
use super::problem::{FILE_GENERATOR_NAME, load_problem, normalize_work_dir, resolve_path};
use anyhow::{Context, Result};
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const INPUT_DIR: &str = "fixtures/input";
const VALIDATOR_DIR: &str = "fixtures/validator";
const CHECKER_DIR: &str = "fixtures/checker";

#[derive(Clone, Debug)]
pub struct AddInputFixtureOptions {
    pub work_dir: PathBuf,
    pub name: String,
    pub from: Option<PathBuf>,
    pub replace: bool,
}

#[derive(Clone, Debug)]
pub struct AddValidatorFixtureOptions {
    pub work_dir: PathBuf,
    pub expect: JudgeExpectation,
    pub name: String,
    pub from: Option<PathBuf>,
    pub replace: bool,
}

#[derive(Clone, Debug)]
pub struct AddCheckerFixtureOptions {
    pub work_dir: PathBuf,
    pub expect: JudgeExpectation,
    pub name: String,
    pub input_from: Option<PathBuf>,
    pub output_from: Option<PathBuf>,
    pub answer_from: Option<PathBuf>,
    pub replace: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct AddFixtureReport {
    pub work_dir: PathBuf,
    pub paths: Vec<PathBuf>,
    pub replaced: bool,
}

impl AddFixtureReport {
    pub fn summary_lines(&self) -> Vec<String> {
        let action = if self.replaced { "wrote" } else { "created" };
        self.paths
            .iter()
            .map(|path| format!("{action} {}", path.display()))
            .collect()
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct FixtureListReport {
    pub work_dir: PathBuf,
    pub inputs: Vec<InputFixture>,
    pub validators: Vec<ValidatorFixture>,
    pub checkers: Vec<CheckerFixture>,
}

#[derive(Clone, Debug, Serialize)]
pub struct InputFixture {
    pub name: String,
    pub path: PathBuf,
    pub used: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct ValidatorFixture {
    pub name: String,
    pub expect: JudgeExpectation,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Serialize)]
pub struct CheckerFixture {
    pub name: String,
    pub expect: JudgeExpectation,
    pub path: PathBuf,
    pub input_path: PathBuf,
    pub output_path: PathBuf,
    pub answer_path: PathBuf,
}

#[derive(Clone, Debug, Serialize)]
pub struct FixtureCheckReport {
    pub ok: bool,
    pub work_dir: PathBuf,
    pub errors: Vec<FixtureIssue>,
    pub list: FixtureListReport,
}

#[derive(Clone, Debug, Serialize)]
pub struct FixtureIssue {
    pub code: &'static str,
    pub path: PathBuf,
    pub message: String,
}

pub fn add_input_fixture(options: AddInputFixtureOptions) -> Result<AddFixtureReport> {
    let work_dir = normalize_work_dir(&options.work_dir)?;
    let name = validate_fixture_stem(&options.name)?;
    let path = work_dir.join(INPUT_DIR).join(format!("{name}.in"));
    let replaced = write_fixture_file(&work_dir, &path, options.from.as_deref(), options.replace)?;
    Ok(AddFixtureReport {
        work_dir,
        paths: vec![path],
        replaced,
    })
}

pub fn add_validator_fixture(options: AddValidatorFixtureOptions) -> Result<AddFixtureReport> {
    let work_dir = normalize_work_dir(&options.work_dir)?;
    let name = validate_fixture_stem(&options.name)?;
    let path = work_dir
        .join(VALIDATOR_DIR)
        .join(options.expect.as_str())
        .join(format!("{name}.in"));
    let replaced = write_fixture_file(&work_dir, &path, options.from.as_deref(), options.replace)?;
    Ok(AddFixtureReport {
        work_dir,
        paths: vec![path],
        replaced,
    })
}

pub fn add_checker_fixture(options: AddCheckerFixtureOptions) -> Result<AddFixtureReport> {
    let work_dir = normalize_work_dir(&options.work_dir)?;
    let name = validate_fixture_stem(&options.name)?;
    let input_from = options
        .input_from
        .as_deref()
        .context("checker fixture requires --input")?;
    let output_from = options
        .output_from
        .as_deref()
        .context("checker fixture requires --output")?;
    let answer_from = options
        .answer_from
        .as_deref()
        .context("checker fixture requires --answer")?;
    let stem = work_dir
        .join(CHECKER_DIR)
        .join(options.expect.as_str())
        .join(name);
    let input_path = stem.with_extension("in");
    let output_path = stem.with_extension("out");
    let answer_path = stem.with_extension("ans");
    let input_replaced =
        write_fixture_file(&work_dir, &input_path, Some(input_from), options.replace)?;
    let output_replaced =
        write_fixture_file(&work_dir, &output_path, Some(output_from), options.replace)?;
    let answer_replaced =
        write_fixture_file(&work_dir, &answer_path, Some(answer_from), options.replace)?;
    Ok(AddFixtureReport {
        work_dir,
        paths: vec![input_path, output_path, answer_path],
        replaced: input_replaced || output_replaced || answer_replaced,
    })
}

pub fn list_fixtures(work_dir: PathBuf) -> Result<FixtureListReport> {
    let work_dir = normalize_work_dir(&work_dir)?;
    let used_inputs = collect_used_input_fixtures(&work_dir)?;
    let inputs = list_input_fixtures(&work_dir, &used_inputs)?;
    let validators = list_validator_fixtures(&work_dir)?;
    let checkers = list_checker_fixtures(&work_dir)?;
    Ok(FixtureListReport {
        work_dir,
        inputs,
        validators,
        checkers,
    })
}

pub fn check_fixtures(work_dir: PathBuf) -> Result<FixtureCheckReport> {
    let list = list_fixtures(work_dir)?;
    let mut errors = Vec::new();
    for input in &list.inputs {
        if !input.used {
            errors.push(FixtureIssue {
                code: "unused_input_fixture",
                path: input.path.clone(),
                message: "input fixture is not referenced by any `:file` test case".to_string(),
            });
        }
    }
    for checker in &list.checkers {
        for path in [
            &checker.input_path,
            &checker.output_path,
            &checker.answer_path,
        ] {
            if !path.is_file() {
                errors.push(FixtureIssue {
                    code: "incomplete_checker_fixture",
                    path: path.clone(),
                    message: "checker fixture must contain matching .in, .out, and .ans files"
                        .to_string(),
                });
            } else if path.metadata().map(|metadata| metadata.len()).unwrap_or(0) == 0 {
                errors.push(FixtureIssue {
                    code: "empty_checker_fixture_file",
                    path: path.clone(),
                    message: "checker fixture files must be non-empty".to_string(),
                });
            }
        }
    }
    Ok(FixtureCheckReport {
        ok: errors.is_empty(),
        work_dir: list.work_dir.clone(),
        errors,
        list,
    })
}

pub fn validator_fixture_reports(work_dir: PathBuf) -> Result<Vec<ValidatorFixture>> {
    let work_dir = normalize_work_dir(&work_dir)?;
    list_validator_fixtures(&work_dir)
}

pub fn checker_fixture_reports(work_dir: PathBuf) -> Result<Vec<CheckerFixture>> {
    let work_dir = normalize_work_dir(&work_dir)?;
    let fixtures = list_checker_fixtures(&work_dir)?;
    for fixture in &fixtures {
        for path in [
            &fixture.input_path,
            &fixture.output_path,
            &fixture.answer_path,
        ] {
            if !path.is_file() {
                anyhow::bail!(
                    "checker fixture `{}/{}` is incomplete; missing {}",
                    fixture.expect.as_str(),
                    fixture.name,
                    path.display()
                );
            }
            if path.metadata().map(|metadata| metadata.len()).unwrap_or(0) == 0 {
                anyhow::bail!(
                    "checker fixture `{}/{}` contains empty file {}",
                    fixture.expect.as_str(),
                    fixture.name,
                    path.display()
                );
            }
        }
    }
    Ok(fixtures)
}

fn write_fixture_file(
    work_dir: &Path,
    target_path: &Path,
    source_path: Option<&Path>,
    replace: bool,
) -> Result<bool> {
    let existed = target_path.exists();
    if existed && !replace {
        anyhow::bail!(
            "fixture already exists: {}; pass --replace to overwrite",
            target_path.display()
        );
    }
    if let Some(parent) = target_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create fixture dir {}", parent.display()))?;
    }
    let bytes = if let Some(source_path) = source_path {
        let source_path = resolve_path(work_dir, source_path);
        std::fs::read(&source_path)
            .with_context(|| format!("failed to read fixture source {}", source_path.display()))?
    } else {
        Vec::new()
    };
    std::fs::write(target_path, bytes)
        .with_context(|| format!("failed to write fixture {}", target_path.display()))?;
    Ok(existed)
}

fn validate_fixture_stem(name: &str) -> Result<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        anyhow::bail!("fixture name cannot be empty");
    }
    let path = Path::new(trimmed);
    if path.components().count() != 1
        || path.file_name().and_then(|part| part.to_str()) != Some(trimmed)
    {
        anyhow::bail!("fixture name must be a file stem, not a path: `{name}`");
    }
    if path.extension().is_some() {
        anyhow::bail!("fixture name must not include an extension: `{name}`");
    }
    if trimmed == "." || trimmed == ".." || trimmed.contains(['/', '\\']) {
        anyhow::bail!("fixture name must be a safe file stem: `{name}`");
    }
    Ok(trimmed.to_string())
}

fn list_input_fixtures(
    work_dir: &Path,
    used_inputs: &BTreeSet<PathBuf>,
) -> Result<Vec<InputFixture>> {
    let mut fixtures = Vec::new();
    for path in list_files_with_extension(&work_dir.join(INPUT_DIR), "in")? {
        let name = fixture_file_stem(&path)?;
        fixtures.push(InputFixture {
            name,
            used: used_inputs.contains(&normalize_relative_path(work_dir, &path)),
            path,
        });
    }
    Ok(fixtures)
}

fn list_validator_fixtures(work_dir: &Path) -> Result<Vec<ValidatorFixture>> {
    let mut fixtures = Vec::new();
    for expect in [JudgeExpectation::Pass, JudgeExpectation::Fail] {
        for path in
            list_files_with_extension(&work_dir.join(VALIDATOR_DIR).join(expect.as_str()), "in")?
        {
            let name = fixture_file_stem(&path)?;
            fixtures.push(ValidatorFixture { name, expect, path });
        }
    }
    Ok(fixtures)
}

fn list_checker_fixtures(work_dir: &Path) -> Result<Vec<CheckerFixture>> {
    let mut fixtures = Vec::new();
    for expect in [JudgeExpectation::Pass, JudgeExpectation::Fail] {
        let root = work_dir.join(CHECKER_DIR).join(expect.as_str());
        if !root.exists() {
            continue;
        }
        let mut names = BTreeSet::new();
        for entry in std::fs::read_dir(&root)
            .with_context(|| format!("failed to read fixture dir {}", root.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if let Some("in" | "out" | "ans") = path.extension().and_then(|value| value.to_str()) {
                names.insert(fixture_file_stem(&path)?);
            }
        }
        for name in names {
            let stem = root.join(&name);
            fixtures.push(CheckerFixture {
                name,
                expect,
                path: stem.clone(),
                input_path: stem.with_extension("in"),
                output_path: stem.with_extension("out"),
                answer_path: stem.with_extension("ans"),
            });
        }
    }
    fixtures.sort_by(|left, right| {
        (left.expect.as_str(), &left.name).cmp(&(right.expect.as_str(), &right.name))
    });
    Ok(fixtures)
}

fn fixture_file_stem(path: &Path) -> Result<String> {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::to_string)
        .with_context(|| format!("fixture path is not valid UTF-8: {}", path.display()))
}

fn list_files_with_extension(dir: &Path, extension: &str) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    if !dir.exists() {
        return Ok(paths);
    }
    for entry in std::fs::read_dir(dir)
        .with_context(|| format!("failed to read fixture dir {}", dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|value| value.to_str()) == Some(extension) {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

fn collect_used_input_fixtures(work_dir: &Path) -> Result<BTreeSet<PathBuf>> {
    let problem = load_problem(work_dir)?;
    let mut used = BTreeSet::new();
    for bundle in problem.test.bundles.values() {
        for case in &bundle.cases {
            if case.generator_name == FILE_GENERATOR_NAME && case.args.len() == 1 {
                used.insert(normalize_relative_path(
                    work_dir,
                    &resolve_path(work_dir, Path::new(&case.args[0])),
                ));
            }
        }
    }
    Ok(used)
}

fn normalize_relative_path(work_dir: &Path, path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        work_dir.join(path)
    };
    absolute
        .strip_prefix(work_dir)
        .map(Path::to_path_buf)
        .unwrap_or(absolute)
}
