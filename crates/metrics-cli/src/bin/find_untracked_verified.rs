use csv::Reader;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::error::Error;
use std::fs::File;

#[derive(Debug, Deserialize)]
struct VerificationCategory {
    identifier: String,
    display_name: String,
    file_name: String,
    category: String,
    #[allow(dead_code)]
    has_requires: bool,
    #[allow(dead_code)]
    has_ensures: bool,
    has_proof_block: bool,
    has_assume_false: bool,
    is_trivially_verified: bool,
    requires_count: usize,
    ensures_count: usize,
}

#[derive(Debug, Deserialize)]
struct CsvRow {
    function: String,
    module: String,
}

#[derive(Debug, Serialize)]
struct UntrackedVerified {
    display_name: String,
    identifier: String,
    file_name: String,
    category: String,
    is_trivially_verified: bool,
    has_proof_block: bool,
    requires_count: usize,
    ensures_count: usize,
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
        eprintln!("Usage: {} <verification_categories_json> <tracked_csv> <output_json>", args[0]);
        eprintln!();
        eprintln!("Finds verified functions NOT in the CSV tracking list.");
        eprintln!();
        eprintln!("Example:");
        eprintln!("  {} \\", args[0]);
        eprintln!("    verification_categories.json \\");
        eprintln!("    functions_to_track.csv \\");
        eprintln!("    untracked_verified.json");
        std::process::exit(1);
    }
    
    let categories_path = &args[1];
    let csv_path = &args[2];
    let output_path = &args[3];
    
    // Load verification categories
    println!("Loading verification categories from {}...", categories_path);
    let file = File::open(categories_path)?;
    let categories: Vec<VerificationCategory> = serde_json::from_reader(file)?;
    println!("  Loaded {} categorizations", categories.len());
    
    // Load tracked functions
    println!("Loading tracked functions from {}...", csv_path);
    let mut reader = Reader::from_path(csv_path)?;
    let mut tracked = HashSet::new();
    
    for result in reader.deserialize() {
        let row: CsvRow = result?;
        tracked.insert(row.function.clone());
        
        // Also add module::function format
        let key = format!("{}::{}", row.module, row.function);
        tracked.insert(key);
    }
    println!("  Loaded {} tracked functions", tracked.len() / 2); // divide by 2 since we store 2 formats
    
    // Find untracked verified functions
    println!("Finding untracked verified functions...");
    let mut untracked_verified = Vec::new();
    
    for cat in &categories {
        // Check if verified (trivially or with proof, but not assume(false))
        let is_verified = (cat.category == "trivially_verified" || cat.category == "verified_with_proof")
            && !cat.has_assume_false;
        
        if !is_verified {
            continue;
        }
        
        // Check if NOT tracked
        let module = extract_module_from_identifier(&cat.identifier);
        let module_function = format!("{}::{}", module, cat.display_name);
        
        let is_tracked = tracked.contains(&cat.display_name) 
            || tracked.contains(&module_function);
        
        if !is_tracked {
            untracked_verified.push(UntrackedVerified {
                display_name: cat.display_name.clone(),
                identifier: cat.identifier.clone(),
                file_name: cat.file_name.clone(),
                category: cat.category.clone(),
                is_trivially_verified: cat.is_trivially_verified,
                has_proof_block: cat.has_proof_block,
                requires_count: cat.requires_count,
                ensures_count: cat.ensures_count,
            });
        }
    }
    
    // Sort by category, then by name
    untracked_verified.sort_by(|a, b| {
        a.category.cmp(&b.category)
            .then_with(|| a.file_name.cmp(&b.file_name))
            .then_with(|| a.display_name.cmp(&b.display_name))
    });
    
    // Compute statistics
    let total_untracked = untracked_verified.len();
    let trivial_untracked = untracked_verified.iter()
        .filter(|f| f.is_trivially_verified)
        .count();
    let proof_untracked = untracked_verified.iter()
        .filter(|f| f.has_proof_block)
        .count();
    
    // Group by file
    let mut by_file: std::collections::HashMap<String, Vec<&UntrackedVerified>> = std::collections::HashMap::new();
    for func in &untracked_verified {
        by_file.entry(func.file_name.clone())
            .or_default()
            .push(func);
    }
    
    // Write output
    println!("Writing results to {}...", output_path);
    let output_file = File::create(output_path)?;
    serde_json::to_writer_pretty(output_file, &untracked_verified)?;
    
    println!("✓ Done!");
    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!("UNTRACKED VERIFIED FUNCTIONS");
    println!("═══════════════════════════════════════════════════════════════");
    println!();
    println!("Total untracked verified: {}", total_untracked);
    println!("  - Trivially verified: {} ({:.1}%)", 
             trivial_untracked, (trivial_untracked as f64 / total_untracked as f64) * 100.0);
    println!("  - With proof blocks: {} ({:.1}%)", 
             proof_untracked, (proof_untracked as f64 / total_untracked as f64) * 100.0);
    println!();
    
    println!("Breakdown by file (top 15):");
    let mut file_counts: Vec<_> = by_file.iter()
        .map(|(file, funcs)| (file, funcs.len()))
        .collect();
    file_counts.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    
    for (file, count) in file_counts.iter().take(15) {
        println!("  {:3} functions in {}", count, file);
    }
    
    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!("EXAMPLES OF UNTRACKED VERIFIED FUNCTIONS");
    println!("═══════════════════════════════════════════════════════════════");
    println!();
    
    // Show examples
    println!("Trivially verified (first 15):");
    for func in untracked_verified.iter()
        .filter(|f| f.is_trivially_verified)
        .take(15) {
        println!("  • {} ({})", func.display_name, func.file_name);
        println!("    requires: {}, ensures: {}", func.requires_count, func.ensures_count);
    }
    
    println!();
    println!("With proof blocks (first 15):");
    for func in untracked_verified.iter()
        .filter(|f| f.has_proof_block && !f.display_name.starts_with("lemma_"))
        .take(15) {
        println!("  • {} ({})", func.display_name, func.file_name);
        println!("    requires: {}, ensures: {}", func.requires_count, func.ensures_count);
    }
    
    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!();
    println!("💡 These are verified functions NOT in your tracking CSV.");
    println!("   Consider adding high-value ones to expand coverage!");
    
    Ok(())
}

