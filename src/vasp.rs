use std::fs::{create_dir_all, File};
use std::io::{self, Error, ErrorKind, Write};
use std::path::Path;
use std::process::Command;

pub struct VaspWorkspace;

impl VaspWorkspace {
    pub fn get_configuration_count(xyz_path: &Path) -> io::Result<usize> {
        let script_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("xyz_to_poscar.py");

        let output = Command::new("python3")
            .arg(&script_path)
            .arg("count")
            .arg(xyz_path)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::new(ErrorKind::Other, format!("Failed to count configurations: {}", stderr.trim())));
        }

        let count_str = String::from_utf8_lossy(&output.stdout);
        count_str
            .trim()
            .parse::<usize>()
            .map_err(|e| Error::new(ErrorKind::InvalidData, format!("Invalid integer count: {}", e)))
    }

    pub fn create_run_directory(
        run_name: &str,
        xyz_path: &Path,
        output_base_dir: &Path,
        setup_dir: &Path,
        config_index: usize,
    ) -> io::Result<()> {
        let run_dir = output_base_dir.join(run_name);
        create_dir_all(&run_dir)?;

        let script_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("xyz_to_poscar.py");

        let output = Command::new("python3")
            .arg(&script_path)
            .arg("convert")
            .arg(xyz_path)
            .arg(run_dir.join("POSCAR"))
            .arg(config_index.to_string())
            .output()?;

        if !output.status.success() {
            return Err(Error::new(ErrorKind::Other, String::from_utf8_lossy(&output.stderr).trim()));
        }

        let vasp_inputs = ["POTCAR", "INCAR", "KPOINTS"];
        for file_name in &vasp_inputs {
            let source = setup_dir.join(file_name);
            let target = run_dir.join(file_name);
            if source.exists() {
                std::fs::copy(&source, &target)?;
            } else {
                return Err(Error::new(ErrorKind::NotFound, format!("Blueprint missing: {}", file_name)));
            }
        }

        Ok(())
    }

    /// Generates a single master Slurm Job Array script for the entire generation.
    pub fn create_array_script(generation_dir: &Path, count: usize) -> io::Result<()> {
        if count == 0 {
            return Err(Error::new(ErrorKind::InvalidInput, "Cannot create an array script for 0 configurations."));
        }

        let array_script_path = generation_dir.join("submit_array.sh");
        let mut file = File::create(&array_script_path)?;

        // Slurm array indices are inclusive (e.g., 50 structures means index 0 to 49)
        let max_index = count - 1;

        // Note the use of %a which Slurm replaces with the padded task ID for logging
        let script_content = format!(
            r#"#!/bin/bash
#SBATCH --job-name=vasp_array
#SBATCH --output=config_%a/vasp.out
#SBATCH --error=config_%a/vasp.err
#SBATCH --nodes=1
#SBATCH --ntasks-per-node=24
#SBATCH --time=02:00:00
#SBATCH --array=0-{}

# Map the 0-indexed task ID to our 3-digit padded folder string (e.g., 3 -> config_003)
DIR=$(printf "config_%03d" $SLURM_ARRAY_TASK_ID)

# Jump into the specific configuration folder and execute VASP
cd $DIR
srun vasp_std
"#,
            max_index
        );

        file.write_all(script_content.as_bytes())?;
        Ok(())
    }
}