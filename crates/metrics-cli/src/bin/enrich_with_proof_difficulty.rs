use csv::{Reader, Writer};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;

#[derive(Debug, Deserialize)]
struct ProofRow {
    function: String,
    #[allow(dead_code)]
    module: String,
    #[allow(dead_code)]
    link: String,
    #[allow(dead_code)]
    has_spec: String,
    has_proof: String,
    trivial_proof: String,
}

#[derive(Debug, Deserialize)]
struct InputRow {
    function: String,
    module: String,
    cyclomatic: Option<String>,
    cognitive: Option<String>,
    halstead_difficulty: Option<String>,
    halstead_effort: Option<String>,
    halstead_length: Option<String>,
    requires_halstead_length: Option<String>,
    requires_halstead_difficulty: Option<String>,
    requires_halstead_effort: Option<String>,
    ensures_halstead_length: Option<String>,
    ensures_halstead_difficulty: Option<String>,
    ensures_halstead_effort: Option<String>,
    decreases_count: Option<String>,
}

#[derive(Debug, Serialize)]
struct OutputRow {
    function: String,
    module: String,
    has_proof: String,
    trivial_proof: String,
    cyclomatic: Option<String>,
    cognitive: Option<String>,
    halstead_difficulty: Option<String>,
    halstead_effort: Option<String>,
    halstead_length: Option<String>,
    requires_halstead_length: Option<String>,
    requires_halstead_difficulty: Option<String>,
    requires_halstead_effort: Option<String>,
    ensures_halstead_length: Option<String>,
    ensures_halstead_difficulty: Option<String>,
    ensures_halstead_effort: Option<String>,
    decreases_count: Option<String>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 {
        eprintln!(
            "Usage: {} <curve25519_with_trivial_csv> <functions_to_track_csv> <output_csv>",
            args[0]
        );
        eprintln!();
        eprintln!("Enriches functions_to_track CSV with proof difficulty information.");
        eprintln!();
        eprintln!("Adds columns:");
        eprintln!("  - has_proof: 'yes' if verified, empty otherwise");
        eprintln!("  - trivial_proof: 'yes' if SMT-verified, 'no' if manual proof, empty if N/A");
        eprintln!();
        eprintln!("Example:");
        eprintln!("  {} \\", args[0]);
        eprintln!("    curve25519_functions_with_trivial.csv \\");
        eprintln!("    functions_to_track_with_specs.csv \\");
        eprintln!("    functions_to_track_complete.csv");
        std::process::exit(1);
    }

    let proof_csv = &args[1];
    let input_csv = &args[2];
    let output_csv = &args[3];

    // Load proof information
    println!("Loading proof information from {}...", proof_csv);
    let mut reader = Reader::from_path(proof_csv)?;

    let mut proof_info: HashMap<String, (String, String)> = HashMap::new();

    for result in reader.deserialize() {
        let row: ProofRow = result?;

        // Store by function name
        proof_info.insert(
            row.function.clone(),
            (row.has_proof.clone(), row.trivial_proof.clone()),
        );

        // Also store by stripped name (e.g., "FieldElement51::add" -> "add")
        if let Some(pos) = row.function.rfind("::") {
            let stripped = row.function[pos + 2..].to_string();
            proof_info
                .entry(stripped)
                .or_insert((row.has_proof.clone(), row.trivial_proof.clone()));
        }
    }

    println!(
        "  Loaded proof info for {} function names",
        proof_info.len()
    );

    // Read input CSV and enrich
    println!("Reading CSV from {}...", input_csv);
    let mut reader = Reader::from_path(input_csv)?;

    let mut enriched_rows = Vec::new();
    let mut stats = Stats::default();

    for result in reader.deserialize() {
        let row: InputRow = result?;
        stats.total += 1;

        // Try to find proof info
        let (has_proof, trivial_proof) = if let Some(info) = proof_info.get(&row.function) {
            stats.matched += 1;
            if info.0 == "yes" {
                stats.with_proof += 1;
                if info.1 == "no" {
                    stats.non_trivial += 1;
                } else if info.1 == "yes" {
                    stats.trivial += 1;
                }
            }
            info.clone()
        } else {
            // Try stripping type prefix
            if let Some(pos) = row.function.rfind("::") {
                let stripped = &row.function[pos + 2..];
                if let Some(info) = proof_info.get(stripped) {
                    stats.matched += 1;
                    if info.0 == "yes" {
                        stats.with_proof += 1;
                        if info.1 == "no" {
                            stats.non_trivial += 1;
                        } else if info.1 == "yes" {
                            stats.trivial += 1;
                        }
                    }
                    info.clone()
                } else {
                    stats.not_found += 1;
                    (String::new(), String::new())
                }
            } else {
                stats.not_found += 1;
                (String::new(), String::new())
            }
        };

        enriched_rows.push(OutputRow {
            function: row.function,
            module: row.module,
            has_proof,
            trivial_proof,
            cyclomatic: row.cyclomatic,
            cognitive: row.cognitive,
            halstead_difficulty: row.halstead_difficulty,
            halstead_effort: row.halstead_effort,
            halstead_length: row.halstead_length,
            requires_halstead_length: row.requires_halstead_length,
            requires_halstead_difficulty: row.requires_halstead_difficulty,
            requires_halstead_effort: row.requires_halstead_effort,
            ensures_halstead_length: row.ensures_halstead_length,
            ensures_halstead_difficulty: row.ensures_halstead_difficulty,
            ensures_halstead_effort: row.ensures_halstead_effort,
            decreases_count: row.decreases_count,
        });
    }

    // Write enriched CSV
    println!("Writing enriched CSV to {}...", output_csv);
    let mut writer = Writer::from_path(output_csv)?;

    for row in &enriched_rows {
        writer.serialize(row)?;
    }
    writer.flush()?;

    println!("✓ Done!");
    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!("PROOF DIFFICULTY ENRICHMENT");
    println!("═══════════════════════════════════════════════════════════════");
    println!();
    println!("Total functions in tracking CSV: {}", stats.total);
    println!(
        "Matched with proof info: {} ({:.1}%)",
        stats.matched,
        (stats.matched as f64 / stats.total as f64) * 100.0
    );
    println!("  ├─ With proofs (has_proof=yes): {}", stats.with_proof);
    println!("  │  ├─ Non-trivial (manual proofs): {}", stats.non_trivial);
    println!("  │  └─ Trivial (SMT-verified): {}", stats.trivial);
    println!("  └─ Without proofs: {}", stats.matched - stats.with_proof);
    println!(
        "Not found: {} ({:.1}%)",
        stats.not_found,
        (stats.not_found as f64 / stats.total as f64) * 100.0
    );
    println!();

    if stats.with_proof > 0 {
        let non_trivial_pct = (stats.non_trivial as f64 / stats.with_proof as f64) * 100.0;
        let trivial_pct = (stats.trivial as f64 / stats.with_proof as f64) * 100.0;

        println!("Of verified functions:");
        println!(
            "  • {:.1}% have non-trivial proofs (manual proof engineering)",
            non_trivial_pct
        );
        println!(
            "  • {:.1}% are trivially verified (SMT handles)",
            trivial_pct
        );
    }

    println!();
    println!("═══════════════════════════════════════════════════════════════");

    Ok(())
}

#[derive(Default)]
struct Stats {
    total: usize,
    matched: usize,
    not_found: usize,
    with_proof: usize,
    non_trivial: usize,
    trivial: usize,
}
