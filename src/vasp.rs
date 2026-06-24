use std::fs::create_dir_all;
use std::io::{self, Error, ErrorKind};
use std::path::Path;
use std::process::Command;

pub struct VaspWorkspace;

impl VaspWorkspace {
    /// Queries the Python ASE subsystem to find out how many frames exist in the target file.
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
            return Err(Error::new(
                ErrorKind::Other,
                format!("Failed to count configurations: {}", stderr.trim()),
            ));
        }

        let count_str = String::from_utf8_lossy(&output.stdout);
        count_str
            .trim()
            .parse::<usize>()
            .map_err(|e| Error::new(ErrorKind::InvalidData, format!("Invalid integer count from script: {}", e)))
    }

    /// Generates a VASP directory structure for an exact configuration frame index.
    pub fn create_run_directory(
        run_name: &str,
        xyz_path: &Path,
        output_base_dir: &Path,
        setup_dir: &Path, // Accepts the whole setup directory now
        config_index: usize,
    ) -> io::Result<()> {
        let run_dir = output_base_dir.join(run_name);
        let poscar_path = run_dir.join("POSCAR");

        create_dir_all(&run_dir)?;

        let script_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("xyz_to_poscar.py");

        // 1. Generate POSCAR
        let output = Command::new("python3")
            .arg(&script_path)
            .arg("convert")
            .arg(xyz_path)
            .arg(&poscar_path)
            .arg(config_index.to_string())
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::new(
                ErrorKind::Other,
                format!("ASE script conversion failed: {}", stderr.trim()),
            ));
        }

        // 2. Dynamically copy all required runtime inputs from your setup blueprint
        let vasp_inputs = ["POTCAR", "INCAR", "KPOINTS"];
        
        for file_name in &vasp_inputs {
            let source_file = setup_dir.join(file_name);
            let target_file = run_dir.join(file_name);

            if source_file.exists() {
                std::fs::copy(&source_file, &target_file)?;
            } else {
                return Err(Error::new(
                    ErrorKind::NotFound,
                    format!("Required VASP input '{}' was not found in setup directory.", file_name),
                ));
            }
        }

        println!("  └─ Workspace prepared: {:?}", run_dir);
        Ok(())
    }
}