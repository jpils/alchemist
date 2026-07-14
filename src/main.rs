mod lammps;
mod vasp;
mod watcher;
mod paths;
mod install;
mod job_template;

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

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum EnergyMode {
    Pet,
    Raw,
}

#[derive(Debug, Deserialize)]
struct CommitteeConfig {
    members: usize,
}

fn main() {

    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 && args[1] == "init" {
        if let Err(e) = install::initialize() {
            eprintln!("❌ {}", e);
        }
        return;
    }
 
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
            eprintln!(
                "Missing job template: {}",
                template_path.display()
            );
            return;
        }
    }

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

    let checkpoint_file: Option<PathBuf> = match config.training.energy_mode {
        EnergyMode::Pet => {
            let mut checkpoint_path: Option<PathBuf> = None;

            if let Ok(entries) = fs::read_dir(&setup_dir) {
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
                    eprintln!("====================================================");
                    eprintln!("❌ PRE-FLIGHT VALIDATION FAILED!");
                    eprintln!(
                        "Energy mode 'pet' requires a checkpoint (*.ckpt) in:"
                    );
                    eprintln!("    {}", setup_dir.display());
                    eprintln!(
                        "Set `energy_mode = \"raw\"` if no PET correction is required."
                    );
                    eprintln!("====================================================");
                    return;
                }
            }
        }

        EnergyMode::Raw => {
            println!(
                " ✓ Raw energy mode selected; no PET checkpoint is required."
            );

            None
        }
    };

    println!(" ✓ All required foundation files validated successfully.");

    let input_file = project_dir.join("filtered_structures.xyz");
    // ==========================================================
    // 🔄 THE MASTER GENERATION LOOP
    // ==========================================================
    for gen_num in 1..=total_generations {
        println!("\n🌀 Starting Generation {}/{}...", gen_num, total_generations);

        println!(
            " ⚙️  Preparing {} committee MD runs...",
            config.committee.members
        );

        let md_generation_dir =
            match LammpsManager::create_generation_workspace(
                &project_dir,
                &setup_dir,
                gen_num,
                config.committee.members,
            ) {
                Ok(path) => path,
                Err(e) => {
                    eprintln!(
                        " ❌ Failed to prepare MD runs for Generation {}: {}",
                        gen_num,
                        e
                    );
                    return;
                }
            };

        let md_array_script = md_generation_dir.join("submit_array.sh");

        println!(
            " 🚀 [Dry-Run] sbatch {:?}",
            md_array_script
        );

        println!(
            " ✓ Prepared {} MD runs for Generation {}.",
            config.committee.members,
            gen_num
        );

        if !input_file.exists() {
            println!(" ⚠️  Generation {} Halted: 'filtered_structures.xyz' not found!", gen_num);
            return;
        }

        let target_base_dir = project_dir
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
                if let Err(e) = VaspWorkspace::create_array_script(
                    &setup_dir,
                    &target_base_dir,
                    gen_num,
                    count,
                ) {
                    eprintln!(
                        " ❌ Failed to generate master Slurm array script: {}",
                        e
                    );
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

                let python_script_path = python_dir.join(
                    config.training.backend.python_script(),
                );

                let pixi_env = config.training.backend.pixi_env();
               
                let energy_mode = match config.training.energy_mode {
                    EnergyMode::Pet => "pet",
                    EnergyMode::Raw => "raw",
                };

                let convert_status = pixi_python(pixi_env)
                    .and_then(|mut cmd| {
                        Ok(cmd.arg(&python_script_path)
                            .arg(&project_dir)
                            .arg(gen_num.to_string())
                            .arg(
                                checkpoint_file
                                    .as_deref()
                                    .unwrap_or_else(|| std::path::Path::new(""))
                            )
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
