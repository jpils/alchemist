mod install;
mod job_template;
mod lammps;
mod paths;
mod training;
mod vasp;
mod watcher;

use lammps::LammpsManager;
use paths::{pixi_python, scheduler_home};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use training::TrainingWorkspace;
use vasp::VaspWorkspace;

#[derive(Debug, Deserialize)]
struct Config {
    project: ProjectConfig,
    training: TrainingConfig,
    committee: CommitteeConfig,
}

#[derive(Debug, Deserialize)]
struct ProjectConfig {
    generations: u32,
}

#[derive(Debug, Deserialize)]
struct TrainingConfig {
    backend: Backend,
    energy_mode: EnergyMode,
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum Backend {
    Upet,
    N2p2,
}

impl Backend {
    pub fn pixi_env(&self) -> &'static str {
        match self {
            Backend::Upet => "upet",
            Backend::N2p2 => "n2p2",
        }
    }

    pub fn python_script(&self) -> &'static str {
        match self {
            Backend::Upet => "poscar_to_upet.py",
            Backend::N2p2 => "poscar_to_n2p2.py",
        }
    }
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum EnergyMode {
    Pet,
    Raw,
}

impl EnergyMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            EnergyMode::Pet => "pet",
            EnergyMode::Raw => "raw",
        }
    }

    pub fn training_key(&self) -> &'static str {
        match self {
            EnergyMode::Pet => "energy-corrected",
            EnergyMode::Raw => "energy",
        }
    }
}

#[derive(Debug, Deserialize)]
struct CommitteeConfig {
    members: usize,
}

fn prepare_training_dataset(
    project_dir: &Path,
    generation: u32,
    backend: Backend,
    checkpoint_file: Option<&Path>,
    energy_mode: EnergyMode,
) -> Result<(), String> {
    let scheduler_dir = scheduler_home().map_err(|error| error.to_string())?;
    let python_script_path = scheduler_dir.join("python").join(backend.python_script());
    let checkpoint_arg = checkpoint_file.unwrap_or_else(|| Path::new(""));

    let status = pixi_python(backend.pixi_env())
        .map_err(|error| format!("Failed to configure Pixi: {}", error))?
        .arg(&python_script_path)
        .arg(project_dir)
        .arg(generation.to_string())
        .arg(checkpoint_arg)
        .arg(energy_mode.as_str())
        .status()
        .map_err(|error| format!("Failed to spawn companion dataset engine: {}", error))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "Python dataset pipeline returned non-zero exit status: {}",
            status
        ))
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 && args[1] == "init" {
        if let Err(e) = install::initialize() {
            eprintln!("❌ {}", e);
        }
        return;
    }

    let project_dir =
        std::env::current_dir().expect("Failed to determine current working directory");

    let setup_dir = project_dir.join("setup");

    if !setup_dir.exists() {
        eprintln!("❌ Could not find setup directory.");
        eprintln!("Expected:");
        eprintln!("    {}", setup_dir.display());
        return;
    }

    let config_path = setup_dir.join("config.toml");

    let config_text = match fs::read_to_string(&config_path) {
        Ok(text) => text,
        Err(e) => {
            eprintln!("❌ Failed to read {}", config_path.display());
            eprintln!("{e}");
            return;
        }
    };

    let config: Config = match toml::from_str(&config_text) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("❌ Invalid config.toml");
            eprintln!("{e}");
            return;
        }
    };

    let total_generations = config.project.generations;

    if config.committee.members == 0 {
        eprintln!("❌ Invalid committee configuration.");
        eprintln!("`committee.members` must be greater than zero.");
        return;
    }

    let required_job_templates = [
        setup_dir.join("jobscripts").join("md_array.sh.template"),
        setup_dir.join("jobscripts").join("vasp_array.sh.template"),
    ];

    for template_path in required_job_templates {
        if !template_path.is_file() {
            eprintln!("❌ PRE-FLIGHT VALIDATION FAILED!");
            eprintln!("Missing job template: {}", template_path.display());
            return;
        }
    }

    if matches!(config.training.backend, Backend::Upet) {
        let required_upet_templates = [
            setup_dir.join("training").join("upet.yaml.template"),
            setup_dir
                .join("jobscripts")
                .join("upet_training_array.sh.template"),
        ];

        for template_path in required_upet_templates {
            if !template_path.is_file() {
                eprintln!("❌ PRE-FLIGHT VALIDATION FAILED!");

                eprintln!("Missing UPET template: {}", template_path.display());

                return;
            }
        }
    }

    // ==========================================================
    // 🔍 PRE-FLIGHT ASSET VALIDATION LOOP
    // ==========================================================
    println!("🔍 Performing pre-flight asset validation...");

    let seed_dataset = {
        let extxyz = setup_dir.join("training").join("seed_dataset.extxyz");
        let xyz = setup_dir.join("training").join("seed_dataset.xyz");

        if extxyz.is_file() {
            extxyz
        } else if xyz.is_file() {
            xyz
        } else {
            eprintln!("❌ PRE-FLIGHT VALIDATION FAILED!");
            eprintln!("Missing seed dataset. Expected one of:");
            eprintln!("    {}", extxyz.display());
            eprintln!("    {}", xyz.display());
            return;
        }
    };

    println!(" ✓ Found seed dataset: {}", seed_dataset.display());

    for vasp_input in ["INCAR", "KPOINTS", "POTCAR"] {
        let path = setup_dir.join("vasp").join(vasp_input);

        if !path.is_file() {
            eprintln!("❌ PRE-FLIGHT VALIDATION FAILED!");
            eprintln!("Missing VASP input: {}", path.display());
            return;
        }
    }

    let lammps_data = setup_dir.join("lammps").join("data").join("lmp.data");

    if !lammps_data.is_file() {
        eprintln!("❌ PRE-FLIGHT VALIDATION FAILED!");
        eprintln!("Missing LAMMPS data file: {}", lammps_data.display());
        return;
    }

    // Check LAMMPS generation files
    for gen_num in 1..=total_generations {
        if let Err(e) = LammpsManager::find_input_file(&setup_dir, gen_num) {
            println!(
                "❌ PRE-FLIGHT VALIDATION FAILED! Gen {} missing input. Details: {}",
                gen_num, e
            );
            return;
        }
    }

    let checkpoint_required = matches!(config.training.backend, Backend::Upet)
        || matches!(config.training.energy_mode, EnergyMode::Pet);

    let checkpoint_file: Option<PathBuf> = if checkpoint_required {
        let mut checkpoint_path = None;
        let training_setup_dir = setup_dir.join("training");

        if let Ok(entries) = fs::read_dir(&training_setup_dir) {
            for entry in entries.flatten() {
                let path = entry.path();

                if path.is_file()
                    && path
                        .extension()
                        .is_some_and(|extension| extension == "ckpt")
                {
                    checkpoint_path = Some(path);
                    break;
                }
            }
        }

        match checkpoint_path {
            Some(path) => {
                println!(
                    " ✓ Found foundation model checkpoint: {:?}",
                    path.file_name().unwrap_or_default()
                );

                Some(path)
            }

            None => {
                eprintln!("❌ A checkpoint (*.ckpt) is required.");
                eprintln!(
                    "UPET training and PET energy correction require a foundation checkpoint in:"
                );
                eprintln!("    {}", training_setup_dir.display());
                return;
            }
        }
    } else {
        println!(" ✓ No foundation checkpoint is required.");

        None
    };

    println!(" ✓ All required foundation files validated successfully.");

    // ==========================================================
    // 🔄 THE MASTER GENERATION LOOP
    // ==========================================================
    for gen_num in 1..=total_generations {
        println!(
            "\n🌀 Starting Generation {}/{}...",
            gen_num, total_generations
        );

        // ==========================================================
        // ⚙️ DATASET PREPARATION
        // ==========================================================
        println!(
            " 🔄 Preparing accumulated dataset for Generation {}...",
            gen_num
        );

        match prepare_training_dataset(
            &project_dir,
            gen_num,
            config.training.backend,
            checkpoint_file.as_deref(),
            config.training.energy_mode,
        ) {
            Ok(()) => {
                println!(" ✓ Generation {} dataset exported successfully.", gen_num);
            }
            Err(error) => {
                eprintln!(" ❌ {}", error);
                return;
            }
        }

        if matches!(config.training.backend, Backend::Upet) {
            let checkpoint = match checkpoint_file.as_deref() {
                Some(path) => path,

                None => {
                    eprintln!(" ❌ UPET training requires a checkpoint.");
                    return;
                }
            };

            println!(" 🧠 Preparing UPET committee training workspace...");

            match TrainingWorkspace::create_upet_workspace(
                &project_dir,
                &setup_dir,
                gen_num,
                config.committee.members,
                checkpoint,
                config.training.energy_mode.training_key(),
            ) {
                Ok(training_script) => {
                    println!(" 🚀 [Dry-Run] sbatch {:?}", training_script);

                    if let Err(error) = TrainingWorkspace::create_mock_upet_models(
                        &project_dir,
                        gen_num,
                        config.committee.members,
                    ) {
                        eprintln!(" ❌ Failed to create mock UPET models: {}", error);
                        return;
                    }

                    println!(" ✓ Created mock UPET trained models.");

                    println!(
                        " ✓ Prepared {} UPET training jobs for Generation {}.",
                        config.committee.members, gen_num
                    );
                }

                Err(error) => {
                    eprintln!(" ❌ Failed to prepare UPET training workspace: {}", error);
                    return;
                }
            }
        }

        println!(
            " ⚙️  Preparing {} committee MD runs...",
            config.committee.members
        );

        let md_generation_dir = match LammpsManager::create_generation_workspace(
            &project_dir,
            &setup_dir,
            gen_num,
            config.committee.members,
            matches!(config.training.backend, Backend::Upet),
        ) {
            Ok(path) => path,
            Err(e) => {
                eprintln!(
                    " ❌ Failed to prepare MD runs for Generation {}: {}",
                    gen_num, e
                );
                return;
            }
        };

        let md_array_script = md_generation_dir.join("submit_array.sh");

        println!(" 🚀 [Dry-Run] sbatch {:?}", md_array_script);

        println!(
            " ✓ Prepared {} MD runs for Generation {}.",
            config.committee.members, gen_num
        );

        let selected_structures = project_dir
            .join("selected_structures")
            .join(format!("generation_{}.xyz", gen_num));

        if !selected_structures.is_file() {
            println!(
                " ⚠️  Generation {} Halted: selected structures file not found: {}",
                gen_num,
                selected_structures.display()
            );
            return;
        }

        let target_base_dir = project_dir
            .join("vasp_runs")
            .join(format!("generation_{}", gen_num));

        match VaspWorkspace::get_configuration_count(&selected_structures) {
            Ok(count) => {
                println!(
                    " 🎯 Found {} selected configurations for Generation {}.",
                    count, gen_num
                );
                println!(" 📂 Populating configuration directories...");

                for i in 0..count {
                    let run_name = format!("config_{:03}", i);
                    let run_dir = target_base_dir.join(&run_name);

                    // 1. Setup the directory structure, copy blueprints, and generate POSCAR
                    if let Err(e) = VaspWorkspace::create_run_directory(
                        &run_name,
                        &selected_structures,
                        &target_base_dir,
                        &setup_dir,
                        i,
                    ) {
                        eprintln!("   ❌ Error building frame {}: {}", i, e);
                        continue;
                    }

                    // 2. Write the mock OUTCAR file right into the folder (Remove/bypass for production)
                    if let Err(e) = VaspWorkspace::create_mock_outcar(&run_dir, i) {
                        eprintln!("   ❌ Error creating mock OUTCAR in {}: {}", run_name, e);
                    }
                }

                // 3. Generate the master Slurm array file
                if let Err(e) =
                    VaspWorkspace::create_array_script(&setup_dir, &target_base_dir, gen_num, count)
                {
                    eprintln!(" ❌ Failed to generate master Slurm array script: {}", e);
                    return;
                }

                let array_script = target_base_dir.join("submit_array.sh");
                println!("   🚀 [Dry-Run] sbatch {:?}", array_script);
                println!(" ✓ Generation {} VASP preparation complete.", gen_num);
            }
            Err(e) => {
                eprintln!(
                    " ❌ Subsystem failure parsing selected structures for Gen {}: {}",
                    gen_num, e
                );
                return;
            }
        }
    }
}
