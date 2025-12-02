use csv::{Reader, Writer};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;
use std::path::PathBuf;

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

fn parse_github_link(link: &str) -> Option<(String, usize)> {
    // Example: https://github.com/Beneficial-AI-Foundation/dalek-lite/blob/main/curve25519-dalek/src/backend/serial/u64/field.rs#L168
    if link.is_empty() {
        return None;
    }
    
    // Extract path after "curve25519-dalek/"
    let parts: Vec<&str> = link.split("curve25519-dalek/").collect();
    if parts.len() < 2 {
        return None;
    }
    
    let file_and_line = parts[1];
    
    // Split by # to separate file path and line number
    let file_parts: Vec<&str> = file_and_line.split('#').collect();
    let file_path = file_parts[0].to_string();
    
    // Extract line number if present
    let line_num = if file_parts.len() > 1 {
        file_parts[1].trim_start_matches('L').parse::<usize>().ok()?
    } else {
        return None;
    };
    
    Some((file_path, line_num))
}

fn has_proof_block(content: &str, start_line: usize) -> Result<bool, String> {
    let lines: Vec<&str> = content.lines().collect();
    
    if start_line == 0 || start_line > lines.len() {
        return Err("Invalid line number".to_string());
    }
    
    // Start from the given line (convert to 0-indexed)
    let start_idx = start_line - 1;
    
    // Find the end of this function (next "fn" keyword or "impl" at same indentation level)
    let mut end_idx = lines.len();
    let mut brace_depth = 0;
    let mut found_opening_brace = false;
    
    for (i, line) in lines.iter().enumerate().skip(start_idx) {
        
        // Track braces
        for ch in line.chars() {
            if ch == '{' {
                brace_depth += 1;
                found_opening_brace = true;
            } else if ch == '}' {
                brace_depth -= 1;
                if found_opening_brace && brace_depth == 0 {
                    // We've closed the function
                    end_idx = i + 1;
                    break;
                }
            }
        }
        
        if found_opening_brace && brace_depth == 0 {
            break;
        }
    }
    
    // Extract the function body
    let function_body: String = lines[start_idx..end_idx].join("\n");
    
    // Check if it contains "proof {" (with space before brace)
    Ok(function_body.contains("proof {"))
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 {
        eprintln!("Usage: {} <source_repo_path> <input_csv> <output_csv>", args[0]);
        eprintln!();
        eprintln!("Adds 'trivial_proof' column by reading source code directly.");
        eprintln!();
        eprintln!("trivial_proof = 'yes' when:");
        eprintln!("  - has_proof = 'yes' but no 'proof {{' block in function body");
        eprintln!();
        eprintln!("trivial_proof = 'no' when:");
        eprintln!("  - has_proof = 'yes' and has 'proof {{' block in function body");
        eprintln!();
        eprintln!("Example:");
        eprintln!("  {} \\", args[0]);
        eprintln!("    /path/to/curve25519-dalek \\");
        eprintln!("    curve25519_functions.csv \\");
        eprintln!("    curve25519_functions_with_trivial.csv");
        std::process::exit(1);
    }
    
    let repo_path = PathBuf::from(&args[1]);
    let input_csv = &args[2];
    let output_csv = &args[3];
    
    if !repo_path.exists() {
        eprintln!("Error: Source repository path does not exist: {}", repo_path.display());
        std::process::exit(1);
    }
    
    println!("Reading source from: {}", repo_path.display());
    println!("Reading CSV from: {}", input_csv);
    
    let mut reader = Reader::from_path(input_csv)?;
    let mut enriched_rows = Vec::new();
    
    let mut stats = Stats::default();
    
    for result in reader.deserialize() {
        let row: InputRow = result?;
        stats.total += 1;
        
        let trivial_proof = if row.has_proof == "yes" {
            stats.has_proof += 1;
            
            if let Some((file_path, line_num)) = parse_github_link(&row.link) {
                let full_path = repo_path.join(&file_path);
                
                if full_path.exists() {
                    match fs::read_to_string(&full_path) {
                        Ok(content) => {
                            match has_proof_block(&content, line_num) {
                                Ok(has_proof) => {
                                    stats.analyzed += 1;
                                    if has_proof {
                                        stats.with_proof_block += 1;
                                        "no" // Has proof block, NOT trivial
                                    } else {
                                        stats.trivial += 1;
                                        "yes" // No proof block, IS trivial
                                    }
                                }
                                Err(e) => {
                                    stats.errors += 1;
                                    eprintln!("Error analyzing {}:{} - {}", file_path, line_num, e);
                                    ""
                                }
                            }
                        }
                        Err(e) => {
                            stats.errors += 1;
                            eprintln!("Error reading {}: {}", full_path.display(), e);
                            ""
                        }
                    }
                } else {
                    stats.file_not_found += 1;
                    eprintln!("File not found: {}", full_path.display());
                    ""
                }
            } else {
                stats.parse_errors += 1;
                eprintln!("Could not parse link: {}", row.link);
                ""
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
    println!("TRIVIAL PROOF ANALYSIS (FROM SOURCE)");
    println!("═══════════════════════════════════════════════════════════════");
    println!();
    println!("Total functions in CSV: {}", stats.total);
    println!("Functions with has_proof=yes: {}", stats.has_proof);
    println!("  ├─ Successfully analyzed: {}", stats.analyzed);
    println!("  │  ├─ Trivially verified (no proof block): {}", stats.trivial);
    println!("  │  └─ With proof blocks: {}", stats.with_proof_block);
    println!("  ├─ Parse errors: {}", stats.parse_errors);
    println!("  ├─ File not found: {}", stats.file_not_found);
    println!("  └─ Analysis errors: {}", stats.errors);
    println!();
    
    if stats.analyzed > 0 {
        let trivial_pct = (stats.trivial as f64 / stats.analyzed as f64) * 100.0;
        let proof_pct = (stats.with_proof_block as f64 / stats.analyzed as f64) * 100.0;
        
        println!("Of analyzed functions with has_proof=yes:");
        println!("  • {:.1}% are trivially verified (no proof block)", trivial_pct);
        println!("  • {:.1}% have proof blocks (manual proofs)", proof_pct);
    }
    
    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!();
    
    // Show examples
    println!("Examples of trivially verified (trivial_proof=yes):");
    for row in enriched_rows.iter().filter(|r| r.trivial_proof == "yes").take(10) {
        println!("  • {} ({})", row.function, row.module);
    }
    
    println!();
    println!("Examples with proof blocks (trivial_proof=no):");
    for row in enriched_rows.iter().filter(|r| r.trivial_proof == "no").take(10) {
        println!("  • {} ({})", row.function, row.module);
    }
    
    Ok(())
}

#[derive(Default)]
struct Stats {
    total: usize,
    has_proof: usize,
    analyzed: usize,
    trivial: usize,
    with_proof_block: usize,
    parse_errors: usize,
    file_not_found: usize,
    errors: usize,
}

