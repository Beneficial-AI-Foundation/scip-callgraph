use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Atom {
    identifier: String,
    statement_type: String,
    deps: Vec<String>,
    body: String,
    display_name: String,
    full_path: String,
    relative_path: String,
    file_name: String,
    parent_folder: String,
    metrics: FunctionMetrics,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct FunctionMetrics {
    requires_count: usize,
    requires_lengths: Vec<usize>,
    ensures_count: usize,
    ensures_lengths: Vec<usize>,
    body_length: usize,
    operators: HashMap<String, usize>,
    
    // New fields from rust-code-analysis (optional for backward compatibility)
    #[serde(skip_serializing_if = "Option::is_none")]
    halstead_length: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cyclomatic: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cognitive: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    halstead_difficulty: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    halstead_effort: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    proof_overhead_direct: Option<i64>,
}

// rust-code-analysis JSON structures
#[derive(Debug, Deserialize)]
struct RcaFile {
    name: String,
    #[serde(default)]
    #[allow(dead_code)]
    start_line: Option<usize>,
    #[serde(default)]
    #[allow(dead_code)]
    end_line: Option<usize>,
    #[serde(default)]
    #[allow(dead_code)]
    kind: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    metrics: Option<RcaMetrics>,
    #[serde(default)]
    spaces: Vec<RcaSpace>,
}

#[derive(Debug, Deserialize)]
struct RcaSpace {
    name: String,
    kind: String,
    #[serde(default)]
    spaces: Vec<RcaSpace>,
    #[serde(default)]
    metrics: Option<RcaMetrics>,
}

#[derive(Debug, Deserialize, Clone)]
struct RcaMetrics {
    #[serde(default)]
    cyclomatic: Option<RcaMetricValue>,
    #[serde(default)]
    cognitive: Option<RcaMetricValue>,
    #[serde(default)]
    halstead: Option<HalsteadMetrics>,
}

#[derive(Debug, Deserialize, Clone)]
struct RcaMetricValue {
    #[serde(default)]
    sum: Option<f64>,
    #[serde(default)]
    #[allow(dead_code)]
    average: Option<f64>,
    #[serde(default)]
    #[allow(dead_code)]
    min: Option<f64>,
    #[serde(default)]
    #[allow(dead_code)]
    max: Option<f64>,
}

#[derive(Debug, Deserialize, Clone)]
struct HalsteadMetrics {
    #[serde(default)]
    length: Option<f64>,
    #[serde(default)]
    difficulty: Option<f64>,
    #[serde(default)]
    effort: Option<f64>,
    #[serde(default)]
    #[allow(dead_code)]
    volume: Option<f64>,
    #[serde(default)]
    #[allow(dead_code)]
    bugs: Option<f64>,
}

fn extract_relative_path(rca_path: &str) -> String {
    // RCA path format: "../curve25519-dalek/curve25519-dalek/src/scalar.rs"
    // We want: "src/scalar.rs"
    
    if let Some(idx) = rca_path.rfind("curve25519-dalek/") {
        let after_last_dalek = &rca_path[idx + "curve25519-dalek/".len()..];
        return after_last_dalek.to_string();
    }
    
    // Fallback: just use the filename
    Path::new(rca_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string()
}

fn extract_function_metrics(space: &RcaSpace) -> Option<(String, RcaMetrics)> {
    if space.kind == "function" {
        if let Some(metrics) = &space.metrics {
            return Some((space.name.clone(), metrics.clone()));
        }
    }
    None
}

fn collect_all_functions(space: &RcaSpace, functions: &mut HashMap<String, RcaMetrics>) {
    // Check current space
    if let Some((name, metrics)) = extract_function_metrics(space) {
        functions.insert(name, metrics);
    }
    
    // Recursively check nested spaces
    for nested in &space.spaces {
        collect_all_functions(nested, functions);
    }
}

fn load_rca_metrics(rca_dir: &str, debug: bool) -> HashMap<String, HashMap<String, RcaMetrics>> {
    // Map: relative_path -> (function_name -> RcaMetrics)
    let mut file_functions: HashMap<String, HashMap<String, RcaMetrics>> = HashMap::new();
    
    // Find all JSON files in rca_dir
    let pattern = format!("{}/**/*.json", rca_dir);
    
    if debug {
        println!("\n[DEBUG] Scanning pattern: {}", pattern);
    }
    
    let mut file_count = 0;
    for entry in glob::glob(&pattern).expect("Failed to read glob pattern") {
        match entry {
            Ok(path) => {
                file_count += 1;
                let path_str = path.display().to_string();
                
                if let Ok(content) = fs::read_to_string(&path) {
                    match serde_json::from_str::<RcaFile>(&content) {
                        Ok(rca_file) => {
                            let rel_path = extract_relative_path(&rca_file.name);
                            let mut functions = HashMap::new();
                            
                            for space in &rca_file.spaces {
                                collect_all_functions(space, &mut functions);
                            }
                            
                            if debug && !functions.is_empty() {
                                println!("[DEBUG] File {}: {} -> {} functions", 
                                    file_count, path_str, functions.len());
                                println!("        RCA name field: {}", rca_file.name);
                                println!("        Extracted rel_path: {}", rel_path);
                                
                                // Show first 3 functions
                                let func_names: Vec<_> = functions.keys().take(3).collect();
                                println!("        First functions: {:?}", func_names);
                            }
                            
                            if !functions.is_empty() {
                                file_functions.insert(rel_path.clone(), functions);
                            }
                        }
                        Err(e) => {
                            if debug {
                                eprintln!("[DEBUG] Failed to parse JSON: {}", path_str);
                                eprintln!("        Error: {}", e);
                            }
                        }
                    }
                } else if debug {
                    eprintln!("[DEBUG] Failed to read file: {}", path_str);
                }
            }
            Err(e) => eprintln!("[ERROR] Glob error: {}", e),
        }
    }
    
    if debug {
        println!("\n[DEBUG] Loaded {} RCA files total", file_functions.len());
        println!("[DEBUG] Files with functions:");
        for (rel_path, funcs) in file_functions.iter().take(5) {
            println!("  - {}: {} functions", rel_path, funcs.len());
        }
    }
    
    file_functions
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    
    let debug = args.contains(&"--debug".to_string()) || args.contains(&"-d".to_string());
    
    if args.len() < 4 {
        eprintln!("Usage: {} <atoms_with_metrics_json> <rca_output_dir> <output_json> [--debug]", args[0]);
        eprintln!("\nArguments:");
        eprintln!("  <atoms_with_metrics_json> - Input atoms JSON with Verus metrics");
        eprintln!("  <rca_output_dir>          - Directory with rust-code-analysis JSONs");
        eprintln!("  <output_json>             - Output path for merged JSON");
        eprintln!("  --debug, -d               - Enable debug output");
        eprintln!("\nExample:");
        eprintln!("  {} curve_dalek_atoms_with_metrics.json curve25519-dalek/ output.json --debug", args[0]);
        std::process::exit(1);
    }
    
    let atoms_path = &args[1];
    let rca_dir = &args[2];
    let output_path = &args[3];
    
    println!("=== Merge rust-code-analysis Metrics ===");
    if debug {
        println!("[DEBUG MODE ENABLED]");
    }
    println!();
    
    // Load atoms JSON
    println!("Loading atoms from {}...", atoms_path);
    let atoms_content = fs::read_to_string(atoms_path)
        .unwrap_or_else(|e| {
            eprintln!("Failed to read atoms JSON: {}", e);
            std::process::exit(1);
        });
    
    let mut atoms: Vec<Atom> = serde_json::from_str(&atoms_content)
        .unwrap_or_else(|e| {
            eprintln!("Failed to parse atoms JSON: {}", e);
            std::process::exit(1);
        });
    
    println!("  Loaded {} functions from atoms JSON", atoms.len());
    
    if debug {
        // Show sample atoms
        println!("\n[DEBUG] Sample atoms (first 3):");
        for atom in atoms.iter().take(3) {
            println!("  - {} ({})", atom.display_name, atom.relative_path);
        }
    }
    
    // Load rust-code-analysis metrics
    println!("\nLoading rust-code-analysis metrics from {}...", rca_dir);
    let rca_metrics = load_rca_metrics(rca_dir, debug);
    println!("  Loaded metrics for {} files", rca_metrics.len());
    
    // Merge metrics
    println!("\nMatching functions...");
    let mut matched = 0;
    let mut unmatched = 0;
    let mut unmatched_examples: Vec<(String, String)> = Vec::new();
    
    for atom in &mut atoms {
        let rel_path = &atom.relative_path;
        let func_name = &atom.display_name;
        
        if let Some(file_functions) = rca_metrics.get(rel_path) {
            if let Some(rca_m) = file_functions.get(func_name) {
                // Extract metrics (handle nested Options)
                let cyclomatic = rca_m.cyclomatic.as_ref().and_then(|m| m.sum);
                let cognitive = rca_m.cognitive.as_ref().and_then(|m| m.sum);
                let halstead_length = rca_m.halstead.as_ref().and_then(|h| h.length);
                let halstead_difficulty = rca_m.halstead.as_ref().and_then(|h| h.difficulty);
                let halstead_effort = rca_m.halstead.as_ref().and_then(|h| h.effort);
                
                // Compute proof_overhead_direct = body_length - halstead_length
                let proof_overhead_direct = if let Some(h_len) = halstead_length {
                    Some(atom.metrics.body_length as i64 - h_len as i64)
                } else {
                    None
                };
                
                // Update atom metrics
                atom.metrics.cyclomatic = cyclomatic;
                atom.metrics.cognitive = cognitive;
                atom.metrics.halstead_length = halstead_length;
                atom.metrics.halstead_difficulty = halstead_difficulty;
                atom.metrics.halstead_effort = halstead_effort;
                atom.metrics.proof_overhead_direct = proof_overhead_direct;
                
                matched += 1;
                
                if debug && matched <= 3 {
                    println!("[DEBUG] MATCHED: {} in {} -> cyclomatic={:.1}", 
                        func_name, rel_path, cyclomatic.unwrap_or(0.0));
                }
            } else {
                unmatched += 1;
                if unmatched_examples.len() < 10 {
                    unmatched_examples.push((func_name.clone(), rel_path.clone()));
                }
                
                if debug && unmatched <= 5 {
                    println!("[DEBUG] UNMATCHED: {} not found in RCA file {}", func_name, rel_path);
                    println!("        Available functions: {:?}", 
                        file_functions.keys().take(5).collect::<Vec<_>>());
                }
            }
        } else {
            unmatched += 1;
            if unmatched_examples.len() < 10 {
                unmatched_examples.push((func_name.clone(), rel_path.clone()));
            }
            
            if debug && unmatched <= 5 {
                println!("[DEBUG] UNMATCHED: File {} not found in RCA metrics", rel_path);
                println!("        Available files: {:?}", 
                    rca_metrics.keys().take(5).collect::<Vec<_>>());
            }
        }
    }
    
    println!("\n=== Matching Results ===");
    println!("  Matched:   {} functions ({:.1}%)", matched, (matched as f64 / atoms.len() as f64) * 100.0);
    println!("  Unmatched: {} functions ({:.1}%)", unmatched, (unmatched as f64 / atoms.len() as f64) * 100.0);
    
    if !unmatched_examples.is_empty() {
        println!("\n  Example unmatched functions:");
        for (func, file) in unmatched_examples.iter().take(10) {
            println!("    - {} in {}", func, file);
        }
    }
    
    // Write output
    println!("\nWriting merged metrics to {}...", output_path);
    let output_json = serde_json::to_string_pretty(&atoms)
        .unwrap_or_else(|e| {
            eprintln!("Failed to serialize output: {}", e);
            std::process::exit(1);
        });
    
    fs::write(output_path, output_json)
        .unwrap_or_else(|e| {
            eprintln!("Failed to write output: {}", e);
            std::process::exit(1);
        });
    
    println!("✓ Merge complete!");
    
    // Summary statistics
    let with_all_metrics = atoms.iter().filter(|a| {
        a.metrics.cyclomatic.is_some() 
        && a.metrics.cognitive.is_some() 
        && a.metrics.halstead_length.is_some()
    }).count();
    
    println!("\n=== Summary ===");
    println!("  Functions with complete metrics: {}/{}", with_all_metrics, atoms.len());
    
    if with_all_metrics > 0 {
        // Find an example with proof overhead
        if let Some(example) = atoms.iter().find(|a| a.metrics.proof_overhead_direct.unwrap_or(0) > 100) {
            println!("\n  Example function with proof overhead:");
            println!("    Name: {}", example.display_name);
            println!("    File: {}", example.relative_path);
            println!("    Body length (with proofs): {}", example.metrics.body_length);
            println!("    Halstead length (without proofs): {:.0}", example.metrics.halstead_length.unwrap_or(0.0));
            println!("    Proof overhead: {}", example.metrics.proof_overhead_direct.unwrap_or(0));
            println!("    Cyclomatic: {:.0}", example.metrics.cyclomatic.unwrap_or(0.0));
            println!("    Cognitive: {:.0}", example.metrics.cognitive.unwrap_or(0.0));
        }
    }
}

