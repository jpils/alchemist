mod vasp;

use std::env;
use std::path::Path;
use vasp::VaspWorkspace;

fn main() {
    let args: Vec<String> = env::args().collect();

    // We expect: <path_to_setup_directory> <total_generations_to_execute>
    if args.len() < 3 {
        println!("====================================================");
        println!("❌ Missing Arguments!");
        println!("Usage:   cargo run -- <path_to_setup_directory> <total_generations>");
        println!("Example: cargo run -- ~/my_setup 3");
        println!("====================================================");
        return;
    }

    let setup_dir = Path::new(&args[1]);

    // Parse how many generations the loop should run total
    let total_generations = match args[2].parse::<u32>() {
        Ok(num) => num,
        Err(_) => {
            eprintln!("❌ Error: Total generations must be a valid integer (e.g., 3, 5, 10).");
            return;
        }
    };

    // Guard Check 1: Verify all blueprint files exist in the setup folder
    let required_files = ["POTCAR", "INCAR", "KPOINTS"];
    for file_name in &required_files {
        if !setup_dir.join(file_name).exists() {
            eprintln!("❌ Configuration Error: '{}' missing from setup path.", file_name);
            return;
        }
    }

    let current_working_dir = env::current_dir().expect("Failed to get current directory");
    
    // Updated to expect a standard .xyz file extension
    let input_file = current_working_dir.join("filtered_structures.xyz");

    println!("--- 🔄 Autonomous Active Learning Multi-Generation Loop ---");
    println!("Resource Blueprint: {:?}", setup_dir);
    println!("Execution Directory: {:?}", current_working_dir);
    println!("Total Planned Iterations: {}", total_generations);
    println!("------------------------------------------------------------");

    // ==========================================================
    // 🔄 THE MASTER GENERATION LOOP
    // ==========================================================
    for gen_num in 1..=total_generations {
        println!("\n🌀 Starting Generation {}/{}...", gen_num, total_generations);

        // ------------------------------------------------------
        // 🛠️ PLACEHOLDER FOR FUTURE WORK
        // This is where your future code will live to launch the MD engine,
        // filter structures, and overwrite 'filtered_structures.xyz'.
        // ------------------------------------------------------
        println!(" ⚙️  [Placeholder] Running MD & ML selection...");
        // ------------------------------------------------------

        // Guard Check 2: Ensure the file exists before VASP processing
        if !input_file.exists() {
            println!(
                " ⚠️  Generation {} Halted: 'filtered_structures.xyz' not found!", 
                gen_num
            );
            return;
        }

        // Route this iteration's outputs to its own distinct folder
        let target_base_dir = current_working_dir
            .join("vasp_runs")
            .join(format!("generation_{}", gen_num));

        // Process the static .xyz file for this generation
        match VaspWorkspace::get_configuration_count(&input_file) {
            Ok(count) => {
                println!(" 🎯 Found {} configurations for Generation {}.", count, gen_num);
                println!(" 📂 Generating VASP workspaces inside: {:?}", target_base_dir);

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
                println!(" ✓ Generation {} workspace setup complete.", gen_num);
            }
            Err(e) => {
                eprintln!(" ❌ Subsystem failure parsing input file in Gen {}: {}", gen_num, e);
                return;
            }
        }
    }

    println!("\n🏁 Loop complete! All {} generations processed autonomously.", total_generations);
}