pub mod test;

use super::program::Program;
use crate::core::problem as core_problem;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::thread;
use test::Test;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Problem {
    pub name: String,
    pub programs: HashMap<String, Program>,
    pub test: Test,
    #[serde(rename = "solution")]
    pub solution_name: String,
    #[serde(rename = "validator")]
    pub validator_name: Option<String>,
    #[serde(rename = "checker")]
    pub checker_name: Option<String>,
}

impl Problem {
    pub fn generate(&self, output_dir: &std::path::PathBuf) -> Result<core_problem::Problem> {
        if output_dir.exists() {
            std::fs::remove_dir_all(output_dir)?;
        }
        std::fs::create_dir_all(output_dir)?;
        let temp_dir = crate::utils::temp_dir();
        if temp_dir.exists() {
            std::fs::remove_dir_all(&temp_dir)?;
        }
        std::fs::create_dir_all(&temp_dir)?;

        let handles = self
            .programs
            .iter()
            .map(|(name, program)| {
                let name = name.clone();
                let program = program.clone();
                let temp_dir = temp_dir.clone();
                thread::spawn(move || {
                    let program = program.generate(&name, &temp_dir)?;
                    Ok::<_, anyhow::Error>((name, program))
                })
            })
            .collect::<Vec<_>>();
        let programs = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Result<HashMap<_, _>>>()?;

        let solution = programs
            .get(&self.solution_name)
            .ok_or_else(|| anyhow::anyhow!("solution `{}` not found", self.solution_name))?;
        let validator = self
            .validator_name
            .as_ref()
            .map(|validator_name| {
                programs
                    .get(validator_name)
                    .ok_or_else(|| anyhow::anyhow!("validator `{}` not found", validator_name))
            })
            .transpose()?;
        let checker = self
            .checker_name
            .as_ref()
            .map(|checker_name| {
                programs
                    .get(checker_name)
                    .ok_or_else(|| anyhow::anyhow!("checker `{}` not found", checker_name))
            })
            .transpose()?;

        let test = self
            .test
            .generate(&programs, solution, validator, output_dir)?;

        let mut used_bundles = std::collections::HashSet::new();
        for task in self.test.tasks.iter() {
            for bundle_name in task.bundles.iter() {
                if !self.test.bundles.contains_key(bundle_name) {
                    return Err(anyhow::anyhow!("test bundle `{}` not found", bundle_name));
                }
                used_bundles.insert(bundle_name);
            }
        }
        for bundle_name in self.test.bundles.keys() {
            if !used_bundles.contains(bundle_name) {
                println!("warning: unused test bundle `{bundle_name}`");
            }
        }

        println!(
            "total score: {:.2}",
            self.test.tasks.iter().map(|task| task.score).sum::<f64>()
        );

        Ok(core_problem::Problem {
            name: self.name.clone(),
            test,
            checker: checker.cloned(),
        })
    }
}
