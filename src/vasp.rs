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

    /// Generates a dynamically sized mock OUTCAR file containing the essential
    /// VASP initialization header tokens required by ASE to parse configurations.
    pub fn create_mock_outcar(run_dir: &Path, config_index: usize) -> io::Result<()> {
        use std::fs::File;
        use std::io::Write;

        let poscar_path = run_dir.join("POSCAR");
        let outcar_path = run_dir.join("OUTCAR");

        // ----------------------------
        // Read POSCAR
        // ----------------------------
        let poscar_content = std::fs::read_to_string(&poscar_path)?;
        let lines: Vec<&str> = poscar_content.lines().collect();

        let mut vrhfin = String::new();
        let mut potcar = String::new();
        let mut ions_per_type = String::from(" ions per type =");
        let mut total_atoms = 0usize;

        if lines.len() >= 7 {
            let elements: Vec<_> = lines[5].split_whitespace().collect();
            let counts: Vec<_> = lines[6].split_whitespace().collect();

            if elements.len() == counts.len() && !elements.is_empty() {
                for (el, count) in elements.iter().zip(counts.iter()) {
                    vrhfin.push_str(&format!("   VRHFIN ={}:\n", el));

                    // Duplicate POTCAR entries (matches real OUTCARs)
                    potcar.push_str(&format!(" POTCAR:    PAW_PBE {} 01Jan2000\n", el));
                    potcar.push_str(&format!(" POTCAR:    PAW_PBE {} 01Jan2000\n", el));

                    ions_per_type.push_str(&format!(" {:>6}", count));

                    total_atoms += count.parse::<usize>().unwrap_or(0);
                }
            }
        }

        // Fallback if POSCAR parsing fails
        if total_atoms == 0 {
            total_atoms = 144;

            // Swapped to S then Cu
            vrhfin = concat!(
                "   VRHFIN =S:\n",
                "   VRHFIN =Cu:\n",
            )
            .to_string();

            potcar = concat!(
                " POTCAR:    PAW_PBE S 01Jan2000\n",
                " POTCAR:    PAW_PBE S 01Jan2000\n",
                " POTCAR:    PAW_PBE Cu 01Jan2000\n",
                " POTCAR:    PAW_PBE Cu 01Jan2000\n",
            )
            .to_string();

            ions_per_type = " ions per type =     48     96".to_string();
        }

        // ----------------------------
        // Mock coordinates/forces
        // ----------------------------
        let mut positions = String::new();

        for _ in 0..total_atoms {
            positions.push_str(
                "   1.00000000   1.00000000   1.00000000   \
    0.00000000   0.00000000   0.00000000\n",
            );
        }

        let energy = -547.509875 - (config_index as f64 * 1.5);

        // ----------------------------
        // OUTCAR
        // ----------------------------
        let outcar = format!(
    r#"vasp.6.4.0 64bit

    {potcar}{vrhfin}{ions_per_type}

    NIONS = {nions:>6}

    direct lattice vectors                 reciprocal lattice vectors
        15.342070    0.000000    0.000000     0.065180    0.000000    0.040625
        1.125002   11.644382    0.000000    -0.006297    0.085878   -0.009843
        -7.397894    0.817018   11.864492     0.000000    0.000000    0.084285

    -----------------------------------------------------------------------------
    Iteration     1(   1)
    -----------------------------------------------------------------------------

    direct lattice vectors                 reciprocal lattice vectors
        15.342070    0.000000    0.000000     0.065180    0.000000    0.040625
        1.125002   11.644382    0.000000    -0.006297    0.085878   -0.009843
        -7.397894    0.817018   11.864492     0.000000    0.000000    0.084285

    POSITION                                       TOTAL-FORCES (eV/Angst)
    -----------------------------------------------------------------------------------
    {positions}-----------------------------------------------------------------------------------

    FREE ENERGIE OF THE ION-ELECTRON SYSTEM (eV)
    ---------------------------------------------------
    free  energy   TOTEN  =      {energy:16.6} eV

    energy  without entropy=     {energy:16.6}  energy(sigma->0) =     {energy:16.6}

    -------------------------------------------------------------------
    General timing and accounting informations for this job:
    -------------------------------------------------------------------

                    Total CPU time used (sec):        0.10
                            User time (sec):            0.08
                        System time (sec):            0.02
                        Elapsed time (sec):            0.10

                    Maximum memory used (kb):      123456
                    Average memory used (kb):      120000

                            Minor page faults:          100
                            Major page faults:            0
                    Voluntary context switches:          12
    "#,
            potcar = potcar,
            vrhfin = vrhfin,
            ions_per_type = ions_per_type,
            nions = total_atoms,
            positions = positions,
            energy = energy,
        );

        let mut file = File::create(outcar_path)?;
        file.write_all(outcar.as_bytes())?;

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