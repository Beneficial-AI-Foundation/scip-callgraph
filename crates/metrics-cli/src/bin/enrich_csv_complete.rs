use csv::{Reader, Writer};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;

#[derive(Debug, Deserialize)]
struct ProofDifficultyRow {
    function: String,
    has_proof: String,
    trivial_proof: String,
}

#[derive(Debug, Deserialize)]
struct AtomWithMetrics {
    identifier: String,
    display_name: String,
    relative_path: String,
    metrics: Metrics,
    proof_metrics: Option<ProofMetrics>,
}

#[derive(Debug, Deserialize)]
struct Metrics {
    requires_specs: Vec<SpecHalstead>,
    ensures_specs: Vec<SpecHalstead>,
    decreases_specs: Vec<SpecHalstead>,
}

#[derive(Debug, Deserialize)]
struct SpecHalstead {
    halstead_length: Option<usize>,
    halstead_difficulty: Option<f64>,
    halstead_effort: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct ProofMetrics {
    direct_proof_halstead: Option<HalsteadCounts>,
    transitive_proof_halstead: Option<HalsteadCounts>,
    proof_depth: usize,
    direct_lemmas: Vec<String>,
    transitive_lemmas: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct HalsteadCounts {
    length: Option<usize>,
    difficulty: Option<f64>,
    effort: Option<f64>,
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
}

#[derive(Debug, Serialize)]
struct OutputRow {
    function: String,
    module: String,
    // Code metrics (existing)
    cyclomatic: Option<String>,
    cognitive: Option<String>,
    halstead_difficulty: Option<String>,
    halstead_effort: Option<String>,
    halstead_length: Option<String>,
    // Proof difficulty (new)
    has_proof: String,
    trivial_proof: String,
    // Spec Halstead metrics (new)
    requires_halstead_length: String,
    requires_halstead_difficulty: String,
    requires_halstead_effort: String,
    ensures_halstead_length: String,
    ensures_halstead_difficulty: String,
    ensures_halstead_effort: String,
    decreases_count: String,
    // Proof Halstead metrics (new)
    direct_proof_length: String,
    direct_proof_difficulty: String,
    direct_proof_effort: String,
    transitive_proof_length: String,
    transitive_proof_difficulty: String,
    transitive_proof_effort: String,
    proof_depth: String,
    direct_lemmas_count: String,
    transitive_lemmas_count: String,
}

fn sum_spec_halstead(specs: &[SpecHalstead]) -> (usize, f64, f64) {
    let length = specs.iter().filter_map(|s| s.halstead_length).sum();
    let difficulty = specs.iter().filter_map(|s| s.halstead_difficulty).sum::<f64>();
    let effort = specs.iter().filter_map(|s| s.halstead_effort).sum::<f64>();
    (length, difficulty, effort)
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 5 {
        eprintln!("Usage: {} <atoms_json> <proof_difficulty_csv> <input_csv> <output_csv>", args[0]);
        eprintln!();
        eprintln!("Enriches CSV with:");
        eprintln!("  1. Proof difficulty (has_proof, trivial_proof)");
        eprintln!("  2. Spec Halstead metrics (requires, ensures, decreases)");
        eprintln!("  3. Proof Halstead metrics (direct, transitive)");
        eprintln!();
        eprintln!("Example:");
        eprintln!("  {} \\", args[0]);
        eprintln!("    curve_dalek_atoms_26_nov_complete.json \\");
        eprintln!("    curve25519_functions_with_trivial.csv \\");
        eprintln!("    functions_to_track_enriched.csv \\");
        eprintln!("    functions_to_track_COMPLETE.csv");
        std::process::exit(1);
    }
    
    let atoms_path = &args[1];
    let proof_diff_csv = &args[2];
    let input_csv = &args[3];
    let output_csv = &args[4];
    
    // Load atoms with metrics
    println!("Loading atoms from {}...", atoms_path);
    let file = File::open(atoms_path)?;
    let atoms: Vec<AtomWithMetrics> = serde_json::from_reader(file)?;
    println!("  Loaded {} atoms", atoms.len());
    
    // Build lookup maps by various name formats
    let mut atoms_by_display: HashMap<String, &AtomWithMetrics> = HashMap::new();
    let mut atoms_by_qualified: HashMap<String, &AtomWithMetrics> = HashMap::new();
    // Map: "display_name|file_path" -> atom (for module-based matching)
    let mut atoms_by_display_and_file: HashMap<String, &AtomWithMetrics> = HashMap::new();
    
    for atom in &atoms {
        atoms_by_display.insert(atom.display_name.clone(), atom);
        
        // Create key combining display_name and file path for module-based matching
        // e.g., "sub|src/backend/serial/u64/field.rs"
        let display_file_key = format!("{}|{}", atom.display_name, atom.relative_path);
        atoms_by_display_and_file.insert(display_file_key, atom);
        
        // Extract Type::method format from identifier
        // Regular method: "4.1.3 field/u64/serial/backend/FieldElement51/as_bytes"
        //   -> parts[-2]=FieldElement51, parts[-1]=as_bytes -> "FieldElement51::as_bytes"
        // Trait impl: "4.1.3 field/u64/serial/backend/FieldElement51/AddAssign/add_assign"
        //   -> parts[-3]=FieldElement51, parts[-2]=AddAssign, parts[-1]=add_assign
        //   -> We want "FieldElement51::add_assign" (Type::method, not Trait::method)
        let parts: Vec<&str> = atom.identifier.split('/').collect();
        if parts.len() >= 2 {
            let method_name = parts[parts.len() - 1];
            let second_last = parts[parts.len() - 2];
            
            // Check if second_last looks like a trait name (PascalCase ending with common trait suffixes)
            // or if method_name matches common trait method patterns
            let is_trait_impl = second_last.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
                && (second_last.ends_with("Assign") 
                    || second_last == "Add" || second_last == "Sub" || second_last == "Mul" || second_last == "Div"
                    || second_last == "Neg" || second_last == "Not"
                    || second_last == "BitAnd" || second_last == "BitOr" || second_last == "BitXor"
                    || second_last == "Shl" || second_last == "Shr"
                    || second_last == "Index" || second_last == "IndexMut"
                    || second_last == "Deref" || second_last == "DerefMut"
                    || second_last == "Drop" || second_last == "Clone" || second_last == "Default"
                    || second_last == "From" || second_last == "Into" || second_last == "TryFrom" || second_last == "TryInto"
                    || second_last == "PartialEq" || second_last == "Eq" || second_last == "PartialOrd" || second_last == "Ord"
                    || second_last == "Hash" || second_last == "Debug" || second_last == "Display");
            
            if is_trait_impl && parts.len() >= 3 {
                // Use the type name (third from last) instead of trait name
                let type_name = parts[parts.len() - 3];
                let qualified = format!("{}::{}", type_name, method_name);
                atoms_by_qualified.insert(qualified, atom);
                
                // Also insert with trait name for completeness
                let trait_qualified = format!("{}::{}", second_last, method_name);
                atoms_by_qualified.entry(trait_qualified).or_insert(atom);
            } else {
                // Regular method
                let qualified = format!("{}::{}", second_last, method_name);
                atoms_by_qualified.insert(qualified, atom);
            }
        }
    }
    
    println!("  Built {} qualified name mappings", atoms_by_qualified.len());
    
    // Load proof difficulty info
    println!("Loading proof difficulty from {}...", proof_diff_csv);
    let mut reader = Reader::from_path(proof_diff_csv)?;
    let mut proof_diff: HashMap<String, (String, String)> = HashMap::new();
    
    for result in reader.deserialize() {
        let row: ProofDifficultyRow = result?;
        proof_diff.insert(row.function.clone(), (row.has_proof, row.trivial_proof));
        
        // Also store stripped name
        if let Some(pos) = row.function.rfind("::") {
            let stripped = row.function[pos+2..].to_string();
            proof_diff.entry(stripped).or_insert((String::new(), String::new()));
        }
    }
    println!("  Loaded {} proof difficulty entries", proof_diff.len());
    
    // Read and enrich CSV
    println!("Reading CSV from {}...", input_csv);
    let mut reader = Reader::from_path(input_csv)?;
    let mut enriched_rows = Vec::new();
    let mut stats = Stats::default();
    
    for result in reader.deserialize() {
        let row: InputRow = result?;
        stats.total += 1;
        
        // Try to find function in atoms using multiple strategies
        let atom = 
            // Strategy 1: Try qualified name match (e.g., "FieldElement51::as_bytes")
            if let Some(a) = atoms_by_qualified.get(&row.function) {
                Some(*a)
            }
            // Strategy 2: Try display name match
            else if let Some(a) = atoms_by_display.get(&row.function) {
                Some(*a)
            }
            // Strategy 3: Strip module prefix and try qualified match
            else if let Some(pos) = row.function.rfind("::") {
                let stripped = &row.function[pos+2..];
                if let Some(a) = atoms_by_qualified.get(stripped) {
                    Some(*a)
                } else {
                    atoms_by_display.get(stripped).copied()
                }
            } else {
                None
            }
            // Strategy 4: Module-based matching using file path
            .or_else(|| {
                // Convert CSV module (e.g., "curve25519_dalek::backend::serial::u64::field")
                // to file path (e.g., "src/backend/serial/u64/field.rs")
                let module_parts: Vec<&str> = row.module.split("::").collect();
                if module_parts.len() >= 2 {
                    // Skip crate name (first part) and convert to path
                    let path_parts: Vec<&str> = module_parts[1..].to_vec();
                    let file_path = format!("src/{}.rs", path_parts.join("/"));
                    
                    // Extract method name from function (e.g., "FieldElement51::sub" -> "sub")
                    let method_name = if let Some(pos) = row.function.rfind("::") {
                        &row.function[pos+2..]
                    } else {
                        &row.function
                    };
                    
                    // Try to find by display_name + file_path
                    let key = format!("{}|{}", method_name, file_path);
                    atoms_by_display_and_file.get(&key).copied()
                } else {
                    None
                }
            });
        
        // Get proof difficulty
        let (has_proof, trivial_proof) = if let Some(info) = proof_diff.get(&row.function) {
            info.clone()
        } else if let Some(pos) = row.function.rfind("::") {
            let stripped = &row.function[pos+2..];
            proof_diff.get(stripped).cloned().unwrap_or((String::new(), String::new()))
        } else {
            (String::new(), String::new())
        };
        
        // Extract metrics if atom found
        let (req_len, req_diff, req_eff, ens_len, ens_diff, ens_eff, dec_count,
             dir_len, dir_diff, dir_eff, trans_len, trans_diff, trans_eff,
             depth, dir_lem, trans_lem) = if let Some(atom) = atom {
            stats.matched += 1;
            
            let (req_len, req_diff, req_eff) = sum_spec_halstead(&atom.metrics.requires_specs);
            let (ens_len, ens_diff, ens_eff) = sum_spec_halstead(&atom.metrics.ensures_specs);
            let dec_count = atom.metrics.decreases_specs.len();
            
            let (dir_len, dir_diff, dir_eff, trans_len, trans_diff, trans_eff, depth, dir_lem, trans_lem) = 
                if let Some(pm) = &atom.proof_metrics {
                    let (dl, dd, de) = if let Some(d) = &pm.direct_proof_halstead {
                        (d.length.unwrap_or(0), d.difficulty.unwrap_or(0.0), d.effort.unwrap_or(0.0))
                    } else {
                        (0, 0.0, 0.0)
                    };
                    
                    let (tl, td, te) = if let Some(t) = &pm.transitive_proof_halstead {
                        (t.length.unwrap_or(0), t.difficulty.unwrap_or(0.0), t.effort.unwrap_or(0.0))
                    } else {
                        (0, 0.0, 0.0)
                    };
                    
                    (dl, dd, de, tl, td, te, pm.proof_depth, pm.direct_lemmas.len(), pm.transitive_lemmas.len())
                } else {
                    (0, 0.0, 0.0, 0, 0.0, 0.0, 0, 0, 0)
                };
            
            (req_len, req_diff, req_eff, ens_len, ens_diff, ens_eff, dec_count,
             dir_len, dir_diff, dir_eff, trans_len, trans_diff, trans_eff,
             depth, dir_lem, trans_lem)
        } else {
            stats.not_found += 1;
            (0, 0.0, 0.0, 0, 0.0, 0.0, 0, 0, 0.0, 0.0, 0, 0.0, 0.0, 0, 0, 0)
        };
        
        enriched_rows.push(OutputRow {
            function: row.function,
            module: row.module,
            cyclomatic: row.cyclomatic,
            cognitive: row.cognitive,
            halstead_difficulty: row.halstead_difficulty,
            halstead_effort: row.halstead_effort,
            halstead_length: row.halstead_length,
            has_proof,
            trivial_proof,
            requires_halstead_length: if req_len > 0 { req_len.to_string() } else { String::new() },
            requires_halstead_difficulty: if req_diff > 0.0 { format!("{:.2}", req_diff) } else { String::new() },
            requires_halstead_effort: if req_eff > 0.0 { format!("{:.2}", req_eff) } else { String::new() },
            ensures_halstead_length: if ens_len > 0 { ens_len.to_string() } else { String::new() },
            ensures_halstead_difficulty: if ens_diff > 0.0 { format!("{:.2}", ens_diff) } else { String::new() },
            ensures_halstead_effort: if ens_eff > 0.0 { format!("{:.2}", ens_eff) } else { String::new() },
            decreases_count: if dec_count > 0 { dec_count.to_string() } else { String::new() },
            direct_proof_length: if dir_len > 0 { dir_len.to_string() } else { String::new() },
            direct_proof_difficulty: if dir_diff > 0.0 { format!("{:.2}", dir_diff) } else { String::new() },
            direct_proof_effort: if dir_eff > 0.0 { format!("{:.2}", dir_eff) } else { String::new() },
            transitive_proof_length: if trans_len > 0 { trans_len.to_string() } else { String::new() },
            transitive_proof_difficulty: if trans_diff > 0.0 { format!("{:.2}", trans_diff) } else { String::new() },
            transitive_proof_effort: if trans_eff > 0.0 { format!("{:.2}", trans_eff) } else { String::new() },
            proof_depth: if depth > 0 { depth.to_string() } else { String::new() },
            direct_lemmas_count: if dir_lem > 0 { dir_lem.to_string() } else { String::new() },
            transitive_lemmas_count: if trans_lem > 0 { trans_lem.to_string() } else { String::new() },
        });
    }
    
    // Write output
    println!("Writing enriched CSV to {}...", output_csv);
    let mut writer = Writer::from_path(output_csv)?;
    for row in &enriched_rows {
        writer.serialize(row)?;
    }
    writer.flush()?;
    
    println!("✓ Done!");
    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!("COMPLETE CSV ENRICHMENT");
    println!("═══════════════════════════════════════════════════════════════");
    println!();
    println!("Total functions: {}", stats.total);
    println!("Matched in atoms: {} ({:.1}%)", stats.matched, (stats.matched as f64 / stats.total as f64) * 100.0);
    println!("Not found: {}", stats.not_found);
    println!();
    println!("New columns added:");
    println!("  • has_proof, trivial_proof (proof difficulty)");
    println!("  • requires_halstead_* (3 metrics)");
    println!("  • ensures_halstead_* (3 metrics)");
    println!("  • decreases_count");
    println!("  • direct_proof_* (3 metrics)");
    println!("  • transitive_proof_* (3 metrics)");
    println!("  • proof_depth, direct_lemmas_count, transitive_lemmas_count");
    println!();
    println!("Total new columns: 16");
    println!("═══════════════════════════════════════════════════════════════");
    
    Ok(())
}

#[derive(Default)]
struct Stats {
    total: usize,
    matched: usize,
    not_found: usize,
}

