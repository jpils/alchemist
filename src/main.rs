mod lammps;
mod vasp;

use std::env;
use std::path::Path;
use lammps::LammpsManager;
use vasp::VaspWorkspace;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 {
        println!("====================================================");
        println!("❌ Missing Arguments!");
        println!("Usage:   cargo run -- <path_to_setup_directory> <total_generations>");
        return;
    }

    let setup_dir = Path::new(&args[1]);
    let total_generations = match args[2].parse::<u32>() {
        Ok(num) => num,
        Err(_) => { eprintln!("❌ Error: Total generations must be an integer."); return; }
    };

    // Pre-Flight Asset Validation Loop
    println!("🔍 Performing pre-flight asset validation...");
    for gen_num in 1..=total_generations {
        if let Err(e) = LammpsManager::find_input_file(setup_dir, gen_num) {
            println!("❌ PRE-FLIGHT VALIDATION FAILED! Gen {} missing input. Details: {}", gen_num, e);
            return;
        }
    }
    println!(" ✓ All required LAMMPS files validated successfully.");

    let current_working_dir = env::current_dir().expect("Failed to get current directory");
    let input_file = current_working_dir.join("filtered_structures.xyz");

    // ==========================================================
    // 🔄 THE MASTER GENERATION LOOP
    // ==========================================================
    for gen_num in 1..=total_generations {
        println!("\n🌀 Starting Generation {}/{}...", gen_num, total_generations);

        let lammps_in_file = LammpsManager::find_input_file(setup_dir, gen_num).unwrap();
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

                // 1. Generate all the distinct config folders and POSCAR copies
                for i in 0..count {
                    let run_name = format!("config_{:03}", i);
                    if let Err(e) = VaspWorkspace::create_run_directory(
                        &run_name,
                        &input_file,
                        &target_base_dir,
                        setup_dir,
                        i,
                    ) {
                        eprintln!("   ❌ Error building frame {}: {}", i, e);
                    }
                }

                // 2. Generate the single master Job Array script in the parent directory 📜
                if let Err(e) = VaspWorkspace::create_array_script(&target_base_dir, count) {
                    eprintln!(" ❌ Failed to generate master Slurm array script: {}", e);
                    return;
                }

                // 3. Print out the single sbatch command for the entire array batch 🚀
                let array_script = target_base_dir.join("submit_array.sh");
                println!("   🚀 [Dry-Run] sbatch {:?}", array_script);
                println!(" ✓ Generation {} processing step complete.", gen_num);
            }
            Err(e) => {
                eprintln!(" ❌ Subsystem failure parsing input file in Gen {}: {}", gen_num, e);
                return;
            }
        }
    }
}