/// Regression test: Verify that all functions in rust-code-analysis JSONs
/// exist in the corresponding source files and in our atoms JSON.
///
/// SETUP:
/// - RCA JSONs: Generated from VANILLA curve25519-dalek 4.1.3 (no Verus)
/// - Atoms JSON: Generated from VERUS curve25519-dalek 4.1.3 (with proofs)
///
/// EXPECTED RELATIONSHIPS:
/// 1. RCA functions → Vanilla source: 100% (RCA ran on vanilla)
/// 2. RCA functions → Atoms (Verus): ~100% (Verus adds to vanilla, doesn't remove)
///    - Exception: cfg-gated functions may differ between builds
/// 3. Atoms (Verus) ⊇ RCA functions (vanilla adds verification code)
///
/// This test ensures:
/// 1. rust-code-analysis ran correctly on vanilla source
/// 2. SCIP indexing captures all vanilla functions in Verus version
/// 3. Our atoms extraction is complete
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

// Type alias to reduce complexity
type FunctionList = Vec<(String, Option<usize>, Option<usize>)>;

#[derive(Debug, Deserialize)]
struct Atom {
    display_name: String,
    relative_path: String,
    #[allow(dead_code)]
    file_name: String,
}

#[derive(Debug, Deserialize)]
struct RcaFile {
    name: String,
    #[serde(default)]
    spaces: Vec<RcaSpace>,
}

#[derive(Debug, Deserialize)]
struct RcaSpace {
    name: String,
    kind: String,
    start_line: Option<usize>,
    end_line: Option<usize>,
    #[serde(default)]
    spaces: Vec<RcaSpace>,
}

fn extract_relative_path(rca_path: &str) -> String {
    // RCA path format: "../curve25519-dalek/curve25519-dalek/src/scalar.rs"
    // or: "curve25519-dalek/src/scalar.rs"
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

fn collect_functions_recursive(space: &RcaSpace, functions: &mut FunctionList) {
    if space.kind == "function" {
        functions.push((space.name.clone(), space.start_line, space.end_line));
    }
    
    for nested in &space.spaces {
        collect_functions_recursive(nested, functions);
    }
}

fn load_rca_functions(rca_dir: &str) -> HashMap<String, FunctionList> {
    // Map: relative_path -> [(function_name, start_line, end_line)]
    let mut file_functions: HashMap<String, FunctionList> = HashMap::new();
    
    let pattern = format!("{}/**/*.json", rca_dir);
    
    for entry in glob::glob(&pattern).expect("Failed to read glob pattern") {
        match entry {
            Ok(path) => {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(rca_file) = serde_json::from_str::<RcaFile>(&content) {
                        let rel_path = extract_relative_path(&rca_file.name);
                        let mut functions = Vec::new();
                        
                        for space in &rca_file.spaces {
                            collect_functions_recursive(space, &mut functions);
                        }
                        
                        if !functions.is_empty() {
                            file_functions.insert(rel_path, functions);
                        }
                    }
                }
            }
            Err(e) => eprintln!("Error reading file: {}", e),
        }
    }
    
    file_functions
}

fn load_atoms_functions(atoms_path: &str) -> HashMap<String, HashSet<String>> {
    // Map: relative_path -> set of function names
    let content = fs::read_to_string(atoms_path)
        .expect("Failed to read atoms JSON");
    
    let atoms: Vec<Atom> = serde_json::from_str(&content)
        .expect("Failed to parse atoms JSON");
    
    let mut file_functions: HashMap<String, HashSet<String>> = HashMap::new();
    
    for atom in atoms {
        file_functions
            .entry(atom.relative_path)
            .or_default()
            .insert(atom.display_name);
    }
    
    file_functions
}

fn check_function_in_source(source_dir: &str, rel_path: &str, func_name: &str, start_line: Option<usize>, end_line: Option<usize>) -> bool {
    let full_path = Path::new(source_dir).join(rel_path);
    
    if let Ok(content) = fs::read_to_string(&full_path) {
        // Simple heuristic: check if "fn func_name" appears in the file
        let pattern1 = format!("fn {}(", func_name);
        let pattern2 = format!("fn {}<", func_name);
        let pattern3 = format!("fn {} ", func_name); // for fn name()
        
        if content.contains(&pattern1) || content.contains(&pattern2) || content.contains(&pattern3) {
            return true;
        }
        
        // Check by line range if available
        if let (Some(start), Some(end)) = (start_line, end_line) {
            let lines: Vec<&str> = content.lines().collect();
            if start > 0 && end <= lines.len() {
                let function_text = lines[start-1..end].join("\n");
                if function_text.contains(&pattern1) || function_text.contains(&pattern2) || function_text.contains(&pattern3) {
                    return true;
                }
            }
        }
    }
    
    false
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 {
        eprintln!("Usage: {} <rca_json_dir> <vanilla_source_dir> <atoms_json>", args[0]);
        eprintln!("\nArguments:");
        eprintln!("  <rca_json_dir>        - Directory with rust-code-analysis JSONs (from vanilla)");
        eprintln!("  <vanilla_source_dir>  - Vanilla source code (NO Verus) that RCA ran on");
        eprintln!("  <atoms_json>          - Atoms JSON from Verus source code");
        eprintln!("\nExample:");
        eprintln!("  {} \\", args[0]);
        eprintln!("    curve25519-dalek/curve25519-dalek \\  # RCA JSONs");
        eprintln!("    /home/lacra/git_repos/curve25519-dalek/curve25519-dalek \\  # Vanilla source");
        eprintln!("    curve_dalek_atoms_with_metrics.json  # Atoms from Verus source");
        eprintln!("\nThis script verifies that:");
        eprintln!("  1. RCA functions exist in vanilla source (should be ~100%)");
        eprintln!("  2. RCA functions exist in atoms from Verus (should be ~100%)");
        eprintln!("  3. Identifies gaps in SCIP indexing or atoms extraction");
        std::process::exit(1);
    }
    
    let rca_dir = &args[1];
    let vanilla_source_dir = &args[2];
    let atoms_path = &args[3];
    
    println!("=== rust-code-analysis Coverage Verification ===");
    println!("RCA JSONs (vanilla):  {}", rca_dir);
    println!("Vanilla source:       {}", vanilla_source_dir);
    println!("Atoms JSON (Verus):   {}", atoms_path);
    println!();
    
    // Load functions from rust-code-analysis JSONs
    println!("Loading rust-code-analysis functions from {}...", rca_dir);
    let rca_functions = load_rca_functions(rca_dir);
    let total_rca_files = rca_functions.len();
    let total_rca_functions: usize = rca_functions.values().map(|v| v.len()).sum();
    println!("  Found {} functions across {} files\n", total_rca_functions, total_rca_files);
    
    // Load functions from atoms JSON
    println!("Loading atoms functions from {}...", atoms_path);
    let atoms_functions = load_atoms_functions(atoms_path);
    let total_atoms_files = atoms_functions.len();
    let _total_atoms_functions: usize = atoms_functions.values().map(|v| v.len()).sum();
    println!("  Found {} functions across {} files\n", total_atoms_files, total_atoms_files);
    
    // Check 1: Verify RCA functions exist in vanilla source code
    println!("=== Check 1: RCA functions exist in vanilla source ===");
    let mut missing_in_vanilla = Vec::new();
    let mut found_in_vanilla = 0;
    
    for (rel_path, functions) in &rca_functions {
        for (func_name, start_line, end_line) in functions {
            if check_function_in_source(vanilla_source_dir, rel_path, func_name, *start_line, *end_line) {
                found_in_vanilla += 1;
            } else {
                missing_in_vanilla.push((rel_path.clone(), func_name.clone()));
            }
        }
    }
    
    println!("  Found in vanilla source: {}/{}", found_in_vanilla, total_rca_functions);
    println!("  Missing from vanilla: {}", missing_in_vanilla.len());
    
    if found_in_vanilla < total_rca_functions {
        println!("  Coverage: {:.1}%", (found_in_vanilla as f64 / total_rca_functions as f64) * 100.0);
    }
    
    if !missing_in_vanilla.is_empty() {
        println!("\n  ⚠️  Functions in RCA but not in vanilla source (first 10):");
        for (file, func) in missing_in_vanilla.iter().take(10) {
            println!("    - {} in {}", func, file);
        }
        println!("  Note: These may be <anonymous> closures or test functions");
    } else {
        println!("  ✓ All RCA functions found in vanilla source!");
    }
    
    // Check 2: Verify RCA functions exist in atoms JSON (from Verus source)
    println!("\n=== Check 2: RCA (vanilla) functions exist in atoms (Verus) ===");
    println!("Expected: ~100% (Verus should include all vanilla functions)");
    let mut missing_in_atoms = Vec::new();
    let mut found_in_atoms = 0;
    
    for (rel_path, functions) in &rca_functions {
        if let Some(atoms_funcs) = atoms_functions.get(rel_path) {
            for (func_name, _, _) in functions {
                if atoms_funcs.contains(func_name) {
                    found_in_atoms += 1;
                } else {
                    missing_in_atoms.push((rel_path.clone(), func_name.clone()));
                }
            }
        } else {
            // Entire file missing from atoms
            for (func_name, _, _) in functions {
                missing_in_atoms.push((rel_path.clone(), func_name.clone()));
            }
        }
    }
    
    println!("  Found in atoms: {}/{}", found_in_atoms, total_rca_functions);
    println!("  Missing from atoms: {}", missing_in_atoms.len());
    
    if !missing_in_atoms.is_empty() {
        println!("\n  ⚠️  Functions in RCA but not in atoms (first 20):");
        for (file, func) in missing_in_atoms.iter().take(20) {
            println!("    - {} in {}", func, file);
        }
        
        // Group by file
        let mut by_file: HashMap<String, Vec<String>> = HashMap::new();
        for (file, func) in &missing_in_atoms {
            by_file.entry(file.clone()).or_default().push(func.clone());
        }
        
        println!("\n  Missing functions by file:");
        for (file, funcs) in by_file.iter().take(5) {
            println!("    {}: {} functions", file, funcs.len());
        }
    } else {
        println!("  ✓ All RCA functions found in atoms!");
    }
    
    // Summary
    println!("\n=== Summary ===");
    println!("RCA → Vanilla source: {:.1}% ({}/{})", 
        (found_in_vanilla as f64 / total_rca_functions as f64) * 100.0,
        found_in_vanilla, total_rca_functions);
    println!("RCA → Atoms (Verus):  {:.1}% ({}/{})", 
        (found_in_atoms as f64 / total_rca_functions as f64) * 100.0,
        found_in_atoms, total_rca_functions);
    
    let vanilla_ok = found_in_vanilla as f64 / total_rca_functions as f64 > 0.90;
    let atoms_ok = found_in_atoms as f64 / total_rca_functions as f64 > 0.90;
    
    println!("\nStatus:");
    println!("  Vanilla check: {}", if vanilla_ok { "✓ PASS (>90%)" } else { "✗ FAIL (<90%)" });
    println!("  Atoms check:   {}", if atoms_ok { "✓ PASS (>90%)" } else { "✗ FAIL (<90%)" });
    
    if !atoms_ok {
        println!("\n⚠️  CRITICAL: {:.0}% of vanilla functions missing from Verus atoms!", 
            ((total_rca_functions - found_in_atoms) as f64 / total_rca_functions as f64) * 100.0);
        println!("This suggests SCIP indexing or atoms extraction issues.");
    }
    
    if vanilla_ok && atoms_ok {
        println!("\n✓ PASS: Coverage is acceptable!");
        std::process::exit(0);
    } else {
        println!("\n✗ FAIL: Coverage below threshold!");
        std::process::exit(1);
    }
}

