use std::fs;
use std::path::{Path, PathBuf};

use crate::job_template::{render_job_template, render_template};

pub struct TrainingWorkspace;

impl TrainingWorkspace {
    pub fn create_upet_workspace(
        project_dir: &Path,
        setup_dir: &Path,
        generation: u32,
        committee_members: usize,
        checkpoint: &Path,
        energy_key: &str,
    ) -> Result<PathBuf, String> {
        if committee_members == 0 {
            return Err("Cannot prepare UPET training for zero committee members.".to_string());
        }

        if !checkpoint.is_file() {
            return Err(format!(
                "UPET checkpoint does not exist: {}",
                checkpoint.display()
            ));
        }

        let generation_dir = project_dir
            .join("training")
            .join(format!("generation_{generation}"));

        let dataset_dir = generation_dir.join("dataset");

        let models_dir = generation_dir.join("models");

        let train_set = dataset_dir.join("train.extxyz");

        let validation_set = dataset_dir.join("validation.extxyz");

        let test_set = dataset_dir.join("test.extxyz");

        for dataset_path in [&train_set, &validation_set, &test_set] {
            if !dataset_path.is_file() {
                return Err(format!(
                    "Required UPET dataset file is missing: {}",
                    dataset_path.display()
                ));
            }
        }

        let yaml_template = setup_dir.join("training").join("upet.yaml.template");

        if !yaml_template.is_file() {
            return Err(format!(
                "Missing UPET training template: {}",
                yaml_template.display()
            ));
        }

        let job_template = setup_dir
            .join("jobscripts")
            .join("upet_training_array.sh.template");

        if !job_template.is_file() {
            return Err(format!(
                "Missing UPET training job template: {}",
                job_template.display()
            ));
        }

        fs::create_dir_all(&models_dir).map_err(|error| {
            format!(
                "Failed to create UPET model directory {}: {}",
                models_dir.display(),
                error
            )
        })?;

        let checkpoint_string = absolute_path_string(checkpoint)?;

        let train_string = absolute_path_string(&train_set)?;

        let validation_string = absolute_path_string(&validation_set)?;

        let test_string = absolute_path_string(&test_set)?;

        for member_index in 0..committee_members {
            let member_dir = models_dir.join(format!("member_{member_index:03}"));

            fs::create_dir_all(&member_dir).map_err(|error| {
                format!(
                    "Failed to create committee member directory {}: {}",
                    member_dir.display(),
                    error
                )
            })?;

            let member_yaml = member_dir.join("train.yaml");

            render_template(
                &yaml_template,
                &member_yaml,
                &[
                    ("checkpoint", checkpoint_string.clone()),
                    ("train_set", train_string.clone()),
                    ("validation_set", validation_string.clone()),
                    ("test_set", test_string.clone()),
                    ("energy_key", energy_key.to_string()),
                ],
            )
            .map_err(|error| {
                format!(
                    "Failed to render UPET configuration for member_{member_index:03}: {}",
                    error
                )
            })?;
        }

        let max_index = committee_members - 1;

        let job_script = generation_dir.join("submit_training_array.sh");

        render_job_template(&job_template, &job_script, generation, max_index)
            .map_err(|error| format!("Failed to render UPET training job script: {}", error))?;

        Ok(job_script)
    }
}

fn absolute_path_string(path: &Path) -> Result<String, String> {
    let absolute = path.canonicalize().map_err(|error| {
        format!(
            "Failed to resolve absolute path {}: {}",
            path.display(),
            error
        )
    })?;

    Ok(absolute.display().to_string())
}
