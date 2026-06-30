use std::fs;
use std::path::{Path, PathBuf};

pub struct LammpsManager;

impl LammpsManager {
    /// Dynamic routing engine for LAMMPS input files.
    /// - If 1 file exists: uses it as a global fallback.
    /// - If >1 file exists: explicitly expects "gen_{gen_num}.in".
    pub fn find_input_file(setup_dir: &Path, gen_num: u32) -> Result<PathBuf, String> {
        let in_dir = setup_dir.join("in");
        
        if !in_dir.exists() || !in_dir.is_dir() {
            return Err(format!("The 'in/' folder is missing inside your setup directory: {:?}", in_dir));
        }

        // Collect all valid files ending with ".in"
        let mut in_files = Vec::new();
        if let Ok(entries) = fs::read_dir(&in_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().map_or(false, |ext| ext == "in") {
                    in_files.push(path);
                }
            }
        }

        if in_files.is_empty() {
            return Err(format!("No files ending in '.in' were found inside {:?}", in_dir));
        }

        // SCENARIO A: Only one file exists -> Use it for every generation unconditionally
        if in_files.len() == 1 {
            return Ok(in_files[0].clone());
        }

        // SCENARIO B: Multiple files exist -> Enforce explicit "gen_X.in" naming schema
        let expected_filename = format!("gen_{}.in", gen_num);
        let specific_path = in_dir.join(&expected_filename);

        if specific_path.exists() {
            Ok(specific_path)
        } else {
            Err(format!(
                "Multiple '.in' files detected, but couldn't find the explicit match for this loop: '{}'",
                expected_filename
            ))
        }
    }
}