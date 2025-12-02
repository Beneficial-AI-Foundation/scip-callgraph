use csv::{Reader, Writer};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;

#[derive(Debug, Deserialize)]
struct VerificationCategory {
    identifier: String,
    display_name: String,
    #[allow(dead_code)]
    category: String,
    has_proof_block: bool,
    is_trivially_verified: bool,
}

#[derive(Debug, Deserialize)]
struct InputRow {
    function: String,
    module: String,
    link: String,
    has_spec: String,
    has_proof: String,
}

#[derive(Debug, Serialize)]
struct OutputRow {
    function: String,
    module: String,
    link: String,
    has_spec: String,
    has_proof: String,
    trivial_proof: String,
}

fn extract_module_from_identifier(identifier: &str) -> String {
    // identifier format: "4.1.3 module/path/Type/function"
    if let Some(path_part) = identifier.split_whitespace().nth(1) {
        let parts: Vec<&str> = path_part.split('/').collect();
        if parts.len() >= 2 {
            return parts[..parts.len()-1].join("::");
        }
    }
    String::new()
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 {
        eprintln!("Usage: {} <verification_categories_json> <input_csv> <output_csv>", args[0]);
        eprintln!();
        eprintln!("Adds 'trivial_proof' column to CSV based on verification analysis.");
        eprintln!();
        eprintln!("trivial_proof = true when:");
        eprintln!("  - has_proof = 'yes' but function is trivially verified (no proof block)");
        eprintln!("  - SMT solver handles verification automatically");
        eprintln!();
        eprintln!("Example:");
        eprintln!("  {} \\", args[0]);
        eprintln!("    verification_categories.json \\");
        eprintln!("    curve25519_functions.csv \\");
        eprintln!("    curve25519_functions_with_trivial.csv");
        std::process::exit(1);
    }
    
    let categories_path = &args[1];
    let input_csv = &args[2];
    let output_csv = &args[3];
    
    // Load verification categories
    println!("Loading verification categories from {}...", categories_path);
    let file = File::open(categories_path)?;
    let categories: Vec<VerificationCategory> = serde_json::from_reader(file)?;
    println!("  Loaded {} categorizations", categories.len());
    
    // Build lookup maps
    let mut by_display_name: HashMap<String, &VerificationCategory> = HashMap::new();
    let mut by_module_function: HashMap<String, &VerificationCategory> = HashMap::new();
    
    for cat in &categories {
        by_display_name.insert(cat.display_name.clone(), cat);
        
        let module = extract_module_from_identifier(&cat.identifier);
        let key = format!("{}::{}", module, cat.display_name);
        by_module_function.insert(key, cat);
    }
    
    println!("  Built lookup maps");
    
    // Read and enrich CSV
    println!("Reading CSV from {}...", input_csv);
    let mut reader = Reader::from_path(input_csv)?;
    
    let mut enriched_rows = Vec::new();
    let mut stats = Stats::default();
    
    for result in reader.deserialize() {
        let row: InputRow = result?;
        stats.total += 1;
        
        // Try to find matching category
        // Strategy 1: Exact match
        let category = if let Some(cat) = by_display_name.get(&row.function) {
            Some(*cat)
        } else {
            // Strategy 2: Strip type prefix (e.g., "FieldElement51::add" -> "add")
            let stripped = if let Some(pos) = row.function.rfind("::") {
                &row.function[pos+2..]
            } else {
                &row.function
            };
            
            if let Some(cat) = by_display_name.get(stripped) {
                Some(*cat)
            } else {
                // Strategy 3: Try module::function
                let key = format!("{}::{}", row.module, row.function);
                by_module_function.get(&key).copied()
            }
        };
        
        // Determine trivial_proof value
        let trivial_proof = if row.has_proof == "yes" {
            stats.has_proof += 1;
            
            if let Some(cat) = category {
                stats.matched += 1;
                
                // If has_proof=yes but is_trivially_verified=true, then it's trivial!
                if cat.is_trivially_verified {
                    stats.trivial_proof += 1;
                    "yes"
                } else if cat.has_proof_block {
                    stats.with_proof_block += 1;
                    "no"
                } else {
                    // Category says not trivial and no proof block - unclear
                    stats.unclear += 1;
                    ""
                }
            } else {
                stats.not_found += 1;
                "" // Unknown
            }
        } else {
            "" // Not applicable (no proof claim)
        };
        
        enriched_rows.push(OutputRow {
            function: row.function,
            module: row.module,
            link: row.link,
            has_spec: row.has_spec,
            has_proof: row.has_proof,
            trivial_proof: trivial_proof.to_string(),
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
    println!("TRIVIAL PROOF ANALYSIS");
    println!("═══════════════════════════════════════════════════════════════");
    println!();
    println!("Total functions in CSV: {}", stats.total);
    println!("Functions with has_proof=yes: {}", stats.has_proof);
    println!("  ├─ Matched in verification data: {}", stats.matched);
    println!("  │  ├─ Trivially verified (trivial_proof=yes): {}", stats.trivial_proof);
    println!("  │  └─ With proof blocks (trivial_proof=no): {}", stats.with_proof_block);
    println!("  ├─ Not found in verification data: {}", stats.not_found);
    println!("  └─ Unclear: {}", stats.unclear);
    println!();
    
    if stats.matched > 0 {
        let trivial_pct = (stats.trivial_proof as f64 / stats.matched as f64) * 100.0;
        let proof_pct = (stats.with_proof_block as f64 / stats.matched as f64) * 100.0;
        
        println!("Of matched functions with has_proof=yes:");
        println!("  • {:.1}% are trivially verified (SMT handles)", trivial_pct);
        println!("  • {:.1}% have actual proof blocks (manual proofs)", proof_pct);
    }
    
    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!();
    
    // Show examples
    println!("Examples of trivially verified functions (trivial_proof=yes):");
    for row in enriched_rows.iter().filter(|r| r.trivial_proof == "yes").take(10) {
        println!("  • {} ({})", row.function, row.module);
    }
    
    println!();
    println!("Examples of functions with proof blocks (trivial_proof=no):");
    for row in enriched_rows.iter().filter(|r| r.trivial_proof == "no").take(10) {
        println!("  • {} ({})", row.function, row.module);
    }
    
    Ok(())
}

#[derive(Default)]
struct Stats {
    total: usize,
    has_proof: usize,
    matched: usize,
    trivial_proof: usize,
    with_proof_block: usize,
    not_found: usize,
    unclear: usize,
}

