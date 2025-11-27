/// Full Metrics Pipeline Runner
///
/// Orchestrates the complete metrics pipeline:
/// 1. Generate atoms from SCIP index
/// 2. Compute spec Halstead metrics
/// 3. Compute proof Halstead metrics
/// 4. Enrich CSV with RCA code metrics
/// 5. Enrich CSV with all spec + proof metrics
///
/// Usage:
///   cargo run -p metrics-cli --bin run_full_pipeline -- \
///     --scip data/scip/curve_dalek_index_scip_26_nov.json \
///     --csv data/csv/functions_to_track.csv \
///     --rca-dir curve25519-dalek/curve25519-dalek/ \
///     --proof-csv data/csv/curve25519_functions_with_trivial.csv \
///     --output-dir data/pipeline_output/

use std::env;
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Instant;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    // Parse arguments
    let mut scip_json = String::new();
    let mut base_csv = String::new();
    let mut rca_dir = String::new();
    let mut proof_csv = String::new();
    let mut output_dir = String::from("data/pipeline_output");
    
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--scip" => {
                scip_json = args.get(i + 1).cloned().unwrap_or_default();
                i += 2;
            }
            "--csv" => {
                base_csv = args.get(i + 1).cloned().unwrap_or_default();
                i += 2;
            }
            "--rca-dir" => {
                rca_dir = args.get(i + 1).cloned().unwrap_or_default();
                i += 2;
            }
            "--proof-csv" => {
                proof_csv = args.get(i + 1).cloned().unwrap_or_default();
                i += 2;
            }
            "--output-dir" => {
                output_dir = args.get(i + 1).cloned().unwrap_or_default();
                i += 2;
            }
            "--help" | "-h" => {
                print_usage(&args[0]);
                return;
            }
            _ => {
                eprintln!("Unknown argument: {}", args[i]);
                i += 1;
            }
        }
    }
    
    // Validate required arguments
    if scip_json.is_empty() || base_csv.is_empty() || rca_dir.is_empty() || proof_csv.is_empty() {
        eprintln!("Error: Missing required arguments\n");
        print_usage(&args[0]);
        std::process::exit(1);
    }
    
    // Create output directory
    fs::create_dir_all(&output_dir).expect("Failed to create output directory");
    
    let total_start = Instant::now();
    
    println!("═══════════════════════════════════════════════════════════════");
    println!("           FULL METRICS PIPELINE");
    println!("═══════════════════════════════════════════════════════════════");
    println!();
    println!("Input SCIP:     {}", scip_json);
    println!("Input CSV:      {}", base_csv);
    println!("RCA Directory:  {}", rca_dir);
    println!("Proof CSV:      {}", proof_csv);
    println!("Output Dir:     {}", output_dir);
    println!();
    
    // Define intermediate file paths
    let step1_atoms = format!("{}/step1_atoms.json", output_dir);
    let step2_specs = format!("{}/step2_with_specs.json", output_dir);
    let step3_proofs = format!("{}/step3_with_proofs.json", output_dir);
    let step4_code = format!("{}/step4_with_code.csv", output_dir);
    let final_csv = format!("{}/FINAL.csv", output_dir);
    
    // Step 1: Generate atoms from SCIP
    println!("───────────────────────────────────────────────────────────────");
    println!("STEP 1: Generate atoms from SCIP");
    println!("───────────────────────────────────────────────────────────────");
    let step1_start = Instant::now();
    
    let status = run_binary("write_atoms", &[&scip_json, &step1_atoms]);
    if !status {
        eprintln!("❌ Step 1 failed!");
        std::process::exit(1);
    }
    
    // Sanity check step 1
    let atoms_count = count_json_array(&step1_atoms);
    println!("✓ Generated {} atoms in {:?}", atoms_count, step1_start.elapsed());
    println!();
    
    // Step 2: Compute spec metrics
    println!("───────────────────────────────────────────────────────────────");
    println!("STEP 2: Compute spec Halstead metrics");
    println!("───────────────────────────────────────────────────────────────");
    let step2_start = Instant::now();
    
    let status = run_binary("compute_metrics", &[&step1_atoms, &step2_specs]);
    if !status {
        eprintln!("❌ Step 2 failed!");
        std::process::exit(1);
    }
    
    println!("✓ Computed spec metrics in {:?}", step2_start.elapsed());
    println!();
    
    // Step 3: Compute proof metrics
    println!("───────────────────────────────────────────────────────────────");
    println!("STEP 3: Compute proof Halstead metrics");
    println!("───────────────────────────────────────────────────────────────");
    let step3_start = Instant::now();
    
    let status = run_binary("compute_proof_metrics", &[&step2_specs, &step3_proofs]);
    if !status {
        eprintln!("❌ Step 3 failed!");
        std::process::exit(1);
    }
    
    println!("✓ Computed proof metrics in {:?}", step3_start.elapsed());
    println!();
    
    // Step 4: Enrich CSV with RCA code metrics
    println!("───────────────────────────────────────────────────────────────");
    println!("STEP 4: Enrich CSV with RCA code metrics");
    println!("───────────────────────────────────────────────────────────────");
    let step4_start = Instant::now();
    
    let status = run_binary("enrich_csv_with_metrics", &[&base_csv, &rca_dir, &step4_code]);
    if !status {
        eprintln!("❌ Step 4 failed!");
        std::process::exit(1);
    }
    
    println!("✓ Added code metrics in {:?}", step4_start.elapsed());
    println!();
    
    // Step 5: Enrich CSV with all spec + proof metrics
    println!("───────────────────────────────────────────────────────────────");
    println!("STEP 5: Enrich CSV with spec + proof metrics");
    println!("───────────────────────────────────────────────────────────────");
    let step5_start = Instant::now();
    
    let status = run_binary("enrich_csv_complete", &[&step3_proofs, &proof_csv, &step4_code, &final_csv]);
    if !status {
        eprintln!("❌ Step 5 failed!");
        std::process::exit(1);
    }
    
    // Count final CSV rows
    let final_rows = count_csv_rows(&final_csv);
    let final_cols = count_csv_columns(&final_csv);
    
    println!("✓ Generated final CSV in {:?}", step5_start.elapsed());
    println!();
    
    // Summary
    println!("═══════════════════════════════════════════════════════════════");
    println!("           PIPELINE COMPLETE");
    println!("═══════════════════════════════════════════════════════════════");
    println!();
    println!("Total time: {:?}", total_start.elapsed());
    println!();
    println!("Output files:");
    println!("  • {}", step1_atoms);
    println!("  • {}", step2_specs);
    println!("  • {}", step3_proofs);
    println!("  • {}", step4_code);
    println!("  • {} (FINAL)", final_csv);
    println!();
    println!("Final CSV: {} rows × {} columns", final_rows, final_cols);
    println!();
    println!("═══════════════════════════════════════════════════════════════");
}

fn print_usage(program: &str) {
    eprintln!("Full Metrics Pipeline Runner");
    eprintln!();
    eprintln!("Usage:");
    eprintln!("  {} \\", program);
    eprintln!("    --scip <scip_index.json> \\");
    eprintln!("    --csv <functions_to_track.csv> \\");
    eprintln!("    --rca-dir <rca_output_directory> \\");
    eprintln!("    --proof-csv <proof_difficulty.csv> \\");
    eprintln!("    [--output-dir <output_directory>]");
    eprintln!();
    eprintln!("Arguments:");
    eprintln!("  --scip        Path to SCIP index JSON file");
    eprintln!("  --csv         Path to functions_to_track.csv");
    eprintln!("  --rca-dir     Directory containing rust-code-analysis JSONs");
    eprintln!("  --proof-csv   Path to CSV with has_proof/trivial_proof columns");
    eprintln!("  --output-dir  Output directory (default: data/pipeline_output)");
    eprintln!();
    eprintln!("Example:");
    eprintln!("  {} \\", program);
    eprintln!("    --scip data/scip/curve_dalek_index_scip_26_nov.json \\");
    eprintln!("    --csv data/csv/functions_to_track.csv \\");
    eprintln!("    --rca-dir curve25519-dalek/curve25519-dalek/ \\");
    eprintln!("    --proof-csv data/csv/curve25519_functions_with_trivial.csv \\");
    eprintln!("    --output-dir data/pipeline_output");
}

fn run_binary(name: &str, args: &[&str]) -> bool {
    // Try to find the binary in target/debug or target/release
    let binary_paths = [
        format!("target/debug/{}", name),
        format!("target/release/{}", name),
        name.to_string(),
    ];
    
    for binary_path in &binary_paths {
        if Path::new(binary_path).exists() || which::which(name).is_ok() {
            let output = Command::new(binary_path)
                .args(args)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .output();
            
            match output {
                Ok(out) => return out.status.success(),
                Err(_) => continue,
            }
        }
    }
    
    // Fallback: use cargo run
    let mut cargo_args = vec![
        "run".to_string(),
        "-p".to_string(),
        "metrics-cli".to_string(),
        "--bin".to_string(),
        name.to_string(),
        "--".to_string(),
    ];
    cargo_args.extend(args.iter().map(|s| s.to_string()));
    
    let output = Command::new("cargo")
        .args(&cargo_args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .expect("Failed to run cargo");
    
    output.status.success()
}

fn count_json_array(path: &str) -> usize {
    let content = fs::read_to_string(path).unwrap_or_default();
    // Quick count: count top-level objects by counting `"identifier":`
    content.matches("\"identifier\":").count()
}

fn count_csv_rows(path: &str) -> usize {
    let content = fs::read_to_string(path).unwrap_or_default();
    content.lines().count().saturating_sub(1) // Exclude header
}

fn count_csv_columns(path: &str) -> usize {
    let content = fs::read_to_string(path).unwrap_or_default();
    content.lines().next().map(|l| l.split(',').count()).unwrap_or(0)
}

