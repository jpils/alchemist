use std::fs::create_dir_all;
use std::io::{self, Error, ErrorKind};
use std::path::{Path, PathBuf};
use crate::paths::{pixi_python, scheduler_home};
use crate::job_template::render_job_template;

pub struct VaspWorkspace;

impl VaspWorkspace {
    pub fn get_configuration_count(xyz_path: &Path) -> io::Result<usize> {
        let script_path = scheduler_home()
            .map_err(|e| Error::new(ErrorKind::Other, e.to_string()))?
            .join("python")
            .join("xyz_to_poscar.py");

        let output = pixi_python("upet")
            .map_err(|e| Error::new(ErrorKind::Other, e.to_string()))?
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

        // POSCAR destination
        let poscar_path = run_dir.join("POSCAR");

        // Python extraction script
        let script_path = scheduler_home()
            .map_err(|e| Error::new(ErrorKind::Other, e.to_string()))?
            .join("python")
            .join("xyz_to_poscar.py");

        // Extract the requested configuration into the POSCAR
        let status = pixi_python("upet")
            .map_err(|e| Error::new(ErrorKind::Other, e.to_string()))?
            .arg(&script_path)
            .arg("extract")
            .arg(xyz_path)
            .arg(config_index.to_string())
            .arg(&poscar_path)
            .status()?;

        if !status.success() {
            return Err(Error::new(
                ErrorKind::Other,
                "xyz_to_poscar.py failed while extracting the POSCAR.",
            ));
        }

        // Copy the standard VASP input files
        let vasp_inputs = ["POTCAR", "INCAR", "KPOINTS"];

        for file_name in &vasp_inputs {
            let source = setup_dir.join(file_name);
            let target = run_dir.join(file_name);

            if source.exists() {
                std::fs::copy(&source, &target)?;
            } else {
                return Err(Error::new(
                    ErrorKind::NotFound,
                    format!("Blueprint missing: {}", file_name),
                ));
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
    pub fn create_array_script(
        setup_dir: &Path,
        generation_dir: &Path,
        generation: u32,
        count: usize,
    ) -> io::Result<PathBuf> {
        if count == 0 {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "Cannot create a VASP array script for zero configurations.",
            ));
        }

        let template_path = setup_dir
            .join("jobscripts")
            .join("vasp_array.sh.template");

        if !template_path.is_file() {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!(
                    "Missing VASP job template: {}",
                    template_path.display()
                ),
            ));
        }

        let script_path = generation_dir.join("submit_array.sh");
        let max_index = count - 1;

        render_job_template(
            &template_path,
            &script_path,
            generation,
            max_index,
        )?;

        Ok(script_path)
    }
}