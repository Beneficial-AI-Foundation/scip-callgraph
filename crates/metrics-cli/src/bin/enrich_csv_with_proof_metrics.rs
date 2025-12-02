use csv::{Reader, Writer};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;

#[derive(Debug, Deserialize)]
struct AtomWithProofMetrics {
    #[allow(dead_code)]
    identifier: String,
    #[allow(dead_code)]
    statement_type: String,
    #[allow(dead_code)]
    deps: Vec<String>,
    #[allow(dead_code)]
    body: String,
    display_name: String,
    full_path: String,
    #[allow(dead_code)]
    relative_path: String,
    #[allow(dead_code)]
    file_name: String,
    #[allow(dead_code)]
    parent_folder: String,
    #[allow(dead_code)]
    metrics: serde_json::Value,
    proof_metrics: Option<ProofMetrics>,
}

#[derive(Debug, Deserialize)]
struct ProofMetrics {
    direct_proof_halstead: HalsteadCounts,
    transitive_proof_halstead: HalsteadCounts,
    direct_lemmas: Vec<String>,
    transitive_lemmas: Vec<String>,
    proof_depth: usize,
    #[allow(dead_code)]
    parse_error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct HalsteadCounts {
    #[allow(dead_code)]
    n1: usize,
    #[allow(dead_code)]
    n1_total: usize,
    #[allow(dead_code)]
    n2: usize,
    #[allow(dead_code)]
    n2_total: usize,
    length: usize,
    difficulty: f64,
    #[allow(dead_code)]
    volume: f64,
    effort: f64,
}

#[derive(Debug, Deserialize)]
struct CsvInputRow {
    function: String,
    module: String,
    cyclomatic: String,
    cognitive: String,
    halstead_difficulty: String,
    halstead_effort: String,
    halstead_length: String,
    requires_halstead_length: String,
    requires_halstead_difficulty: String,
    requires_halstead_effort: String,
    ensures_halstead_length: String,
    ensures_halstead_difficulty: String,
    ensures_halstead_effort: String,
    decreases_count: String,
}

#[derive(Debug, Serialize)]
struct CsvOutputRow {
    function: String,
    module: String,
    cyclomatic: String,
    cognitive: String,
    halstead_difficulty: String,
    halstead_effort: String,
    halstead_length: String,
    requires_halstead_length: String,
    requires_halstead_difficulty: String,
    requires_halstead_effort: String,
    ensures_halstead_length: String,
    ensures_halstead_difficulty: String,
    ensures_halstead_effort: String,
    decreases_count: String,
    // New proof metrics columns
    direct_proof_length: String,
    direct_proof_difficulty: String,
    direct_proof_effort: String,
    transitive_proof_length: String,
    transitive_proof_difficulty: String,
    transitive_proof_effort: String,
    proof_overhead: String,
    proof_depth: String,
    direct_lemmas_count: String,
    transitive_lemmas_count: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 {
        eprintln!(
            "Usage: {} <proof_metrics_json> <input_csv> <output_csv>",
            args[0]
        );
        eprintln!();
        eprintln!("Enriches CSV with proof metrics (direct and transitive).");
        eprintln!();
        eprintln!("Example:");
        eprintln!("  {} \\", args[0]);
        eprintln!("    test_proof_metrics.json \\");
        eprintln!("    functions_to_track_with_specs.csv \\");
        eprintln!("    functions_complete.csv");
        std::process::exit(1);
    }

    let proof_json_path = &args[1];
    let input_csv_path = &args[2];
    let output_csv_path = &args[3];

    println!("Loading proof metrics from {}...", proof_json_path);
    let file = File::open(proof_json_path)?;
    let atoms: Vec<AtomWithProofMetrics> = serde_json::from_reader(file)?;
    println!("  Loaded {} functions", atoms.len());

    let with_proofs = atoms.iter().filter(|a| a.proof_metrics.is_some()).count();
    println!("  Functions with proofs: {}", with_proofs);

    // Build lookup maps by full_path only (display_name can have duplicates)
    let mut by_full_path: HashMap<String, &AtomWithProofMetrics> = HashMap::new();

    for atom in &atoms {
        by_full_path.insert(atom.full_path.clone(), atom);
        // Also add display_name as a key for fallback
        by_full_path.insert(atom.display_name.clone(), atom);
    }

    println!("Loading CSV from {}...", input_csv_path);
    let mut reader = Reader::from_path(input_csv_path)?;
    let mut writer = Writer::from_path(output_csv_path)?;

    let mut matched = 0;
    let mut total = 0;
    let mut with_proof_metrics = 0;

    for result in reader.deserialize() {
        let input_row: CsvInputRow = result?;
        total += 1;

        // Try to find the function in the proof metrics JSON
        let function_name = &input_row.function;
        let module = &input_row.module;

        // Strategy 1: Try module::function (most specific, use FIRST)
        let key1 = format!("{}::{}", module, function_name);
        let mut atom_opt: Option<&AtomWithProofMetrics> = by_full_path.get(&key1).copied();

        // Strategy 2: Extract last segment of module and try module::Type::function
        if atom_opt.is_none() && module.contains("::") {
            let parts: Vec<&str> = module.split("::").collect();
            if let Some(last_part) = parts.last() {
                let key = format!("{}::{}::{}", module, last_part, function_name);
                atom_opt = by_full_path.get(&key).copied();
            }
        }

        // Strategy 3: Try matching any full_path that contains module and ends with function
        if atom_opt.is_none() {
            for (full_path, atom) in &by_full_path {
                // Check if full_path contains the module and ends with ::function_name
                if full_path.contains(module)
                    && full_path.ends_with(&format!("::{}", function_name))
                {
                    atom_opt = Some(atom);
                    break;
                }
            }
        }

        // Strategy 4: Try just the function name (last resort, may match wrong function)
        if atom_opt.is_none() {
            atom_opt = by_full_path.get(function_name).copied();
        }

        let output_row = if let Some(atom) = atom_opt {
            matched += 1;

            if let Some(pm) = &atom.proof_metrics {
                with_proof_metrics += 1;

                // Calculate proof overhead ratio
                let proof_overhead = if pm.direct_proof_halstead.effort > 0.0 {
                    pm.transitive_proof_halstead.effort / pm.direct_proof_halstead.effort
                } else {
                    0.0
                };

                CsvOutputRow {
                    function: input_row.function,
                    module: input_row.module,
                    cyclomatic: input_row.cyclomatic,
                    cognitive: input_row.cognitive,
                    halstead_difficulty: input_row.halstead_difficulty,
                    halstead_effort: input_row.halstead_effort,
                    halstead_length: input_row.halstead_length,
                    requires_halstead_length: input_row.requires_halstead_length,
                    requires_halstead_difficulty: input_row.requires_halstead_difficulty,
                    requires_halstead_effort: input_row.requires_halstead_effort,
                    ensures_halstead_length: input_row.ensures_halstead_length,
                    ensures_halstead_difficulty: input_row.ensures_halstead_difficulty,
                    ensures_halstead_effort: input_row.ensures_halstead_effort,
                    decreases_count: input_row.decreases_count,
                    direct_proof_length: pm.direct_proof_halstead.length.to_string(),
                    direct_proof_difficulty: format!("{:.2}", pm.direct_proof_halstead.difficulty),
                    direct_proof_effort: format!("{:.2}", pm.direct_proof_halstead.effort),
                    transitive_proof_length: pm.transitive_proof_halstead.length.to_string(),
                    transitive_proof_difficulty: format!(
                        "{:.2}",
                        pm.transitive_proof_halstead.difficulty
                    ),
                    transitive_proof_effort: format!("{:.2}", pm.transitive_proof_halstead.effort),
                    proof_overhead: format!("{:.2}", proof_overhead),
                    proof_depth: pm.proof_depth.to_string(),
                    direct_lemmas_count: pm.direct_lemmas.len().to_string(),
                    transitive_lemmas_count: pm.transitive_lemmas.len().to_string(),
                }
            } else {
                // Matched but no proof metrics
                CsvOutputRow {
                    function: input_row.function,
                    module: input_row.module,
                    cyclomatic: input_row.cyclomatic,
                    cognitive: input_row.cognitive,
                    halstead_difficulty: input_row.halstead_difficulty,
                    halstead_effort: input_row.halstead_effort,
                    halstead_length: input_row.halstead_length,
                    requires_halstead_length: input_row.requires_halstead_length,
                    requires_halstead_difficulty: input_row.requires_halstead_difficulty,
                    requires_halstead_effort: input_row.requires_halstead_effort,
                    ensures_halstead_length: input_row.ensures_halstead_length,
                    ensures_halstead_difficulty: input_row.ensures_halstead_difficulty,
                    ensures_halstead_effort: input_row.ensures_halstead_effort,
                    decreases_count: input_row.decreases_count,
                    direct_proof_length: String::new(),
                    direct_proof_difficulty: String::new(),
                    direct_proof_effort: String::new(),
                    transitive_proof_length: String::new(),
                    transitive_proof_difficulty: String::new(),
                    transitive_proof_effort: String::new(),
                    proof_overhead: String::new(),
                    proof_depth: String::new(),
                    direct_lemmas_count: String::new(),
                    transitive_lemmas_count: String::new(),
                }
            }
        } else {
            // No match found - keep original data, add empty proof metrics
            CsvOutputRow {
                function: input_row.function,
                module: input_row.module,
                cyclomatic: input_row.cyclomatic,
                cognitive: input_row.cognitive,
                halstead_difficulty: input_row.halstead_difficulty,
                halstead_effort: input_row.halstead_effort,
                halstead_length: input_row.halstead_length,
                requires_halstead_length: input_row.requires_halstead_length,
                requires_halstead_difficulty: input_row.requires_halstead_difficulty,
                requires_halstead_effort: input_row.requires_halstead_effort,
                ensures_halstead_length: input_row.ensures_halstead_length,
                ensures_halstead_difficulty: input_row.ensures_halstead_difficulty,
                ensures_halstead_effort: input_row.ensures_halstead_effort,
                decreases_count: input_row.decreases_count,
                direct_proof_length: String::new(),
                direct_proof_difficulty: String::new(),
                direct_proof_effort: String::new(),
                transitive_proof_length: String::new(),
                transitive_proof_difficulty: String::new(),
                transitive_proof_effort: String::new(),
                proof_overhead: String::new(),
                proof_depth: String::new(),
                direct_lemmas_count: String::new(),
                transitive_lemmas_count: String::new(),
            }
        };

        writer.serialize(output_row)?;
    }

    writer.flush()?;

    println!();
    println!("✓ Done!");
    println!();
    println!("Results:");
    println!("  Total rows: {}", total);
    println!(
        "  Matched: {} ({:.1}%)",
        matched,
        (matched as f64 / total as f64) * 100.0
    );
    println!(
        "  With proof metrics: {} ({:.1}%)",
        with_proof_metrics,
        (with_proof_metrics as f64 / total as f64) * 100.0
    );
    println!("  Unmatched: {}", total - matched);
    println!();
    println!("Output written to: {}", output_csv_path);
    println!();
    println!("New columns added:");
    println!("  • direct_proof_length          (tokens in proof block)");
    println!("  • direct_proof_difficulty      (proof block complexity)");
    println!("  • direct_proof_effort          (proof block effort)");
    println!("  • transitive_proof_length      (proof + lemmas tokens)");
    println!("  • transitive_proof_difficulty  (proof + lemmas complexity)");
    println!("  • transitive_proof_effort      (proof + lemmas effort)");
    println!("  • proof_overhead               (transitive/direct ratio)");
    println!("  • proof_depth                  (lemma call chain depth)");
    println!("  • direct_lemmas_count          (lemmas called directly)");
    println!("  • transitive_lemmas_count      (all lemmas transitively)");

    Ok(())
}
