use csv::{Reader, Writer};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;  // Used for lookup maps and category_counts
use std::error::Error;
use std::fs::File;

#[derive(Debug, Deserialize)]
struct VerificationCategory {
    identifier: String,
    display_name: String,
    file_name: String,
    category: String,
    has_requires: bool,
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
struct EnrichedCsvRow {
    function: String,
    module: String,
    verification_category: String,
    has_proof_block: String,
    is_trivially_verified: String,
    requires_count: String,
    ensures_count: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 {
        eprintln!("Usage: {} <verification_categories_json> <input_csv> <output_csv>", args[0]);
        eprintln!();
        eprintln!("Enriches CSV with verification category information.");
        eprintln!();
        eprintln!("Example:");
        eprintln!("  {} \\", args[0]);
        eprintln!("    verification_categories.json \\");
        eprintln!("    functions_to_track.csv \\");
        eprintln!("    functions_with_verification_categories.csv");
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
        
        // Extract module from identifier (format: "4.1.3 module/path/Type/function")
        if let Some(path_part) = cat.identifier.split_whitespace().nth(1) {
            let parts: Vec<&str> = path_part.split('/').collect();
            if parts.len() >= 2 {
                let function = parts.last().unwrap();
                let module = parts[..parts.len()-1].join("::");
                let key = format!("{}::{}", module, function);
                by_module_function.insert(key, cat);
            }
        }
    }
    
    println!("  Built lookup maps: {} by name, {} by module::function",
             by_display_name.len(), by_module_function.len());
    
    // Read and enrich CSV
    println!("Reading CSV from {}...", input_csv);
    let mut reader = Reader::from_path(input_csv)?;
    let headers = reader.headers()?.clone();
    
    let mut enriched_rows = Vec::new();
    let mut matched = 0;
    let mut total = 0;
    
    for result in reader.deserialize() {
        let row: CsvRow = result?;
        total += 1;
        
        // Try to find matching category
        let category = if let Some(cat) = by_display_name.get(&row.function) {
            Some(*cat)
        } else {
            let key = format!("{}::{}", row.module, row.function);
            by_module_function.get(&key).map(|v| *v)
        };
        
        let enriched = if let Some(cat) = category {
            matched += 1;
            EnrichedCsvRow {
                function: row.function,
                module: row.module,
                verification_category: cat.category.clone(),
                has_proof_block: if cat.has_proof_block { "yes" } else { "no" }.to_string(),
                is_trivially_verified: if cat.is_trivially_verified { "yes" } else { "no" }.to_string(),
                requires_count: cat.requires_count.to_string(),
                ensures_count: cat.ensures_count.to_string(),
            }
        } else {
            EnrichedCsvRow {
                function: row.function,
                module: row.module,
                verification_category: "not_found".to_string(),
                has_proof_block: "N/A".to_string(),
                is_trivially_verified: "N/A".to_string(),
                requires_count: "N/A".to_string(),
                ensures_count: "N/A".to_string(),
            }
        };
        
        enriched_rows.push(enriched);
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
    println!("CSV ENRICHMENT SUMMARY");
    println!("═══════════════════════════════════════════════════════════════");
    println!();
    println!("Total functions in CSV: {}", total);
    println!("Matched with categories: {} ({:.1}%)", matched, (matched as f64 / total as f64) * 100.0);
    println!("Not found: {}", total - matched);
    println!();
    
    // Compute statistics for matched functions
    let mut category_counts: HashMap<String, usize> = HashMap::new();
    for row in &enriched_rows {
        if row.verification_category != "not_found" {
            *category_counts.entry(row.verification_category.clone()).or_insert(0) += 1;
        }
    }
    
    println!("Verification breakdown for CSV functions:");
    let mut categories: Vec<_> = category_counts.iter().collect();
    categories.sort_by_key(|(_, count)| std::cmp::Reverse(**count));
    
    for (cat, count) in categories {
        println!("  {}: {} ({:.1}%)", 
                 cat, count, (*count as f64 / matched as f64) * 100.0);
    }
    
    let trivial = category_counts.get("trivially_verified").unwrap_or(&0);
    let with_proof = category_counts.get("verified_with_proof").unwrap_or(&0);
    let total_verified = trivial + with_proof;
    
    println!();
    println!("TOTAL VERIFIED in CSV: {} ({:.1}%)", 
             total_verified, (total_verified as f64 / matched as f64) * 100.0);
    println!("  - Trivially verified: {} ({:.1}%)", 
             trivial, (*trivial as f64 / total_verified as f64) * 100.0);
    println!("  - With proof blocks: {} ({:.1}%)", 
             with_proof, (*with_proof as f64 / total_verified as f64) * 100.0);
    
    println!();
    println!("═══════════════════════════════════════════════════════════════");
    
    Ok(())
}

