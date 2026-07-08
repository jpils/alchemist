mod lammps;
mod vasp;
mod watcher;
mod paths;

use std::env;
use std::fs;
use std::path::PathBuf;
use lammps::LammpsManager;
use vasp::VaspWorkspace;
use serde::Deserialize;
use paths::{pixi_python, scheduler_home};

#[derive(Debug, Deserialize)]
struct Config {
    project: ProjectConfig,
    training: TrainingConfig,
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Backend {
    Upet,
    N2p2,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum EnergyMode {
    Pet,
    Raw,
}


fn main() {
 
    let project_dir = std::env::current_dir()
        .expect("Failed to determine current working directory");

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

    // ==========================================================
    // 🔍 PRE-FLIGHT ASSET VALIDATION LOOP
    // ==========================================================
    println!("🔍 Performing pre-flight asset validation...");
    
    // Check LAMMPS generation files
    for gen_num in 1..=total_generations {
        if let Err(e) = LammpsManager::find_input_file(&setup_dir, gen_num) {
            println!("❌ PRE-FLIGHT VALIDATION FAILED! Gen {} missing input. Details: {}", gen_num, e);
            return;
        }
    }

    // New: Scan for the foundation model checkpoint (.ckpt) in the setup directory
    let mut checkpoint_path: Option<PathBuf> = None;
    if let Ok(entries) = fs::read_dir(&setup_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |ext| ext == "ckpt") {
                checkpoint_path = Some(path);
                break; // Stop at the first valid checkpoint found
            }
        }
    }

    let checkpoint_file = match checkpoint_path {
        Some(path) => {
            println!(" ✓ Found foundation model checkpoint: {:?}", path.file_name().unwrap());
            path
        }
        None => {
            println!("====================================================");
            println!("❌ PRE-FLIGHT VALIDATION FAILED!");
            println!("Missing model checkpoint file (*.ckpt) inside setup directory: {:?}", setup_dir);
            println!("🛑 Execution safely aborted before running any simulations.");
            println!("====================================================");
            return;
        }
    };

    println!(" ✓ All required foundation files validated successfully.");

    let current_working_dir = env::current_dir().expect("Failed to get current directory");
    let input_file = current_working_dir.join("filtered_structures.xyz");

    // ==========================================================
    // 🔄 THE MASTER GENERATION LOOP
    // ==========================================================
    for gen_num in 1..=total_generations {
        println!("\n🌀 Starting Generation {}/{}...", gen_num, total_generations);

        let lammps_in_file = LammpsManager::find_input_file(&setup_dir, gen_num).unwrap();
        println!(" ⚙️  [Placeholder] Launching MD using: {:?}", lammps_in_file.file_name().unwrap());

        if !input_file.exists() {
            println!(" ⚠️  Generation {} Halted: 'filtered_structures.xyz' not found!", gen_num);
            return;
        }

        let target_base_dir = current_working_dir
            .join("vasp_runs")
            .join(format!("generation_{}", gen_num));

        match VaspWorkspace::get_configuration_count(&input_file) {
            Ok(count) => {
                println!(" 🎯 Found {} configurations for Generation {}.", count, gen_num);
                println!(" 📂 Populating configuration directories...");

                for i in 0..count {
                    let run_name = format!("config_{:03}", i);
                    let run_dir = target_base_dir.join(&run_name);

                    // 1. Setup the directory structure, copy blueprints, and generate POSCAR
                    if let Err(e) = VaspWorkspace::create_run_directory(
                        &run_name,
                        &input_file,
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
                if let Err(e) = VaspWorkspace::create_array_script(&target_base_dir, count) {
                    eprintln!(" ❌ Failed to generate master Slurm array script: {}", e);
                    return;
                }

                let array_script = target_base_dir.join("submit_array.sh");
                println!("   🚀 [Dry-Run] sbatch {:?}", array_script);
                println!(" ✓ Generation {} processing step complete.", gen_num);

                // ==========================================================
                // ⚙️ AUTOMATED CONVERSION TRIGGER
                // ==========================================================
                println!(" 🔄 Invoking automated extraction and dataset formatting step...");

                let scheduler_dir = match scheduler_home() {
                    Ok(dir) => dir,
                    Err(e) => {
                        eprintln!("❌ {}", e);
                        return;
                    }
                };

                let python_dir = scheduler_dir.join("python");

                let python_script_path = match config.training.backend {
                    Backend::Upet => python_dir.join("poscar_to_upet.py"),
                    Backend::N2p2 => python_dir.join("poscar_to_n2p2.py"),
                };

                let pixi_env = match config.training.backend {
                    Backend::Upet => "upet",
                    Backend::N2p2 => "n2p2",
                };
               

                let energy_mode = match config.training.energy_mode {
                    EnergyMode::Pet => "pet",
                    EnergyMode::Raw => "raw",
                };

                let convert_status = pixi_python(pixi_env)
                    .and_then(|mut cmd| {
                        Ok(cmd.arg(&python_script_path)
                            .arg(&project_dir)
                            .arg(gen_num.to_string())
                            .arg(&checkpoint_file)
                            .arg(energy_mode)
                            .status())
                    });

                match convert_status {
                    Ok(Ok(status)) => {
                        if status.success() {
                            println!(
                                " ✓ Generation {} completely compiled and split successfully.",
                                gen_num
                            );
                        } else {
                            eprintln!(
                                "   ⚠️ Python pipeline returned non-zero exit status: {}",
                                status
                            );
                        }
                    }

                    Ok(Err(e)) => {
                        eprintln!("   ❌ Failed to spawn companion extraction engine: {}", e);
                    }

                    Err(e) => {
                        eprintln!("   ❌ Failed to configure Pixi: {}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!(" ❌ Subsystem failure parsing input file in Gen {}: {}", gen_num, e);
                return;
            }
        }
    }
}
