use crate::job_template::render_job_template;
use std::fs;
use std::path::{Path, PathBuf};

pub struct LammpsManager;

impl LammpsManager {
    /// Find the LAMMPS input file for a generation.
    ///
    /// Rules:
    /// - If exactly one `.in` file exists, use it for every generation.
    /// - If multiple `.in` files exist, require `gen_<generation>.in`.
    pub fn find_input_file(setup_dir: &Path, gen_num: u32) -> Result<PathBuf, String> {
        let in_dir = setup_dir.join("in");

        if !in_dir.is_dir() {
            return Err(format!(
                "The 'in/' folder is missing inside the setup directory: {}",
                in_dir.display()
            ));
        }

        let mut in_files = Vec::new();

        let entries = fs::read_dir(&in_dir).map_err(|e| {
            format!(
                "Failed to read LAMMPS input directory {}: {}",
                in_dir.display(),
                e
            )
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                format!("Failed to inspect an entry in {}: {}", in_dir.display(), e)
            })?;

            let path = entry.path();

            if path.is_file() && path.extension().is_some_and(|extension| extension == "in") {
                in_files.push(path);
            }
        }

        in_files.sort();

        if in_files.is_empty() {
            return Err(format!(
                "No files ending in '.in' were found inside {}",
                in_dir.display()
            ));
        }

        if in_files.len() == 1 {
            return Ok(in_files[0].clone());
        }

        let expected_filename = format!("gen_{}.in", gen_num);
        let specific_path = in_dir.join(&expected_filename);

        if specific_path.is_file() {
            Ok(specific_path)
        } else {
            Err(format!(
                "Multiple LAMMPS input files were found, but '{}' is missing",
                expected_filename
            ))
        }
    }

    /// Create the MD committee workspace for one active-learning generation.
    ///
    /// One MD run is created per committee member.
    pub fn create_generation_workspace(
        project_dir: &Path,
        setup_dir: &Path,
        gen_num: u32,
        committee_members: usize,
    ) -> Result<PathBuf, String> {
        if committee_members == 0 {
            return Err("The committee must contain at least one member.".to_string());
        }

        let input_file = Self::find_input_file(setup_dir, gen_num)?;

        let generation_dir = project_dir
            .join("md_runs")
            .join(format!("generation_{}", gen_num));

        fs::create_dir_all(&generation_dir).map_err(|e| {
            format!(
                "Failed to create MD generation directory {}: {}",
                generation_dir.display(),
                e
            )
        })?;

        for run_index in 0..committee_members {
            let run_name = format!("run_{:03}", run_index);
            let run_dir = generation_dir.join(&run_name);

            fs::create_dir_all(&run_dir).map_err(|e| {
                format!(
                    "Failed to create MD run directory {}: {}",
                    run_dir.display(),
                    e
                )
            })?;

            // Each run is driven by the corresponding committee member.
            let member_name = format!("member_{:03}", run_index);

            let model_dir = project_dir
                .join("training")
                .join(format!("generation_{}", gen_num))
                .join("models")
                .join(&member_name);

            // Copy the generation-specific LAMMPS input into the run directory.
            fs::copy(&input_file, run_dir.join("input.lmp")).map_err(|e| {
                format!(
                    "Failed to copy {} into {}: {}",
                    input_file.display(),
                    run_dir.display(),
                    e
                )
            })?;

            // Record the committee member assigned as the MD driver.
            fs::write(
                run_dir.join("driver_member.txt"),
                format!("{member_name}\n"),
            )
            .map_err(|e| format!("Failed to write driver information for {}: {}", run_name, e))?;

            // Record the expected model directory.
            //
            // We do not inject this path into the LAMMPS input yet because
            // UPET and n2p2 use different pair-style configurations.
            fs::write(
                run_dir.join("driver_model_path.txt"),
                format!("{}\n", model_dir.display()),
            )
            .map_err(|e| format!("Failed to write model path for {}: {}", run_name, e))?;
        }

        Self::create_md_array_script(setup_dir, &generation_dir, gen_num, committee_members)?;

        Ok(generation_dir)
    }

    /// Create one Slurm array task per MD run.
    pub fn create_md_array_script(
        setup_dir: &Path,
        generation_dir: &Path,
        gen_num: u32,
        committee_members: usize,
    ) -> Result<PathBuf, String> {
        if committee_members == 0 {
            return Err("Cannot create an MD array for zero committee members.".to_string());
        }

        let template_path = setup_dir.join("jobscripts").join("md_array.sh.template");

        if !template_path.is_file() {
            return Err(format!(
                "Missing MD job template: {}",
                template_path.display()
            ));
        }

        let max_index = committee_members - 1;
        let script_path = generation_dir.join("submit_array.sh");

        render_job_template(&template_path, &script_path, gen_num, max_index).map_err(|error| {
            format!(
                "Failed to render MD job template {}: {}",
                template_path.display(),
                error
            )
        })?;

        Ok(script_path)
    }
}
