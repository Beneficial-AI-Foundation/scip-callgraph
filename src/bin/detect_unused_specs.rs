use scip_callgraph::scip_to_call_graph_json::{
    build_call_graph, parse_scip_json, write_call_graph_as_atoms_json, Atom,
};
use scip_callgraph::scip_utils::generate_scip_json_index;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        eprintln!("Usage: {} <path_to_rust_project> [path_to_scip_json]", args[0]);
        eprintln!("\nExamples:");
        eprintln!("  {} /path/to/project", args[0]);
        eprintln!("  {} /path/to/project /path/to/project_index_scip.json", args[0]);
        eprintln!("\nIf SCIP JSON path is not provided, it will be generated automatically.");
        std::process::exit(1);
    }

    let project_path = &args[1];
    let scip_json_path: String;

    // Step 1: Get or generate SCIP JSON
    if args.len() >= 3 {
        scip_json_path = args[2].clone();
        println!("Using provided SCIP JSON: {}", scip_json_path);
        
        if !Path::new(&scip_json_path).exists() {
            eprintln!("Error: SCIP JSON file '{}' does not exist", scip_json_path);
            std::process::exit(1);
        }
    } else {
        println!("No SCIP JSON provided, generating it...");
        
        scip_json_path = generate_scip_json_index(project_path)?;
        println!("Generated SCIP JSON: {}", scip_json_path);
    }

    // Step 2: Build call graph
    println!("\nParsing SCIP JSON...");
    let scip_data = parse_scip_json(&scip_json_path)?;
    
    println!("Building call graph...");
    let call_graph = build_call_graph(&scip_data);
    println!("Call graph contains {} functions", call_graph.len());

    // Step 3: Write atoms JSON
    let atoms_json_path = format!("{}_atoms.json", 
        Path::new(project_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("project")
    );
    
    println!("Writing atoms JSON to {}...", atoms_json_path);
    write_call_graph_as_atoms_json(&call_graph, &atoms_json_path)?;

    // Step 4: Load atoms JSON and analyze
    println!("Analyzing for unused specs...");
    let unused_specs = find_unused_specs(&atoms_json_path)?;

    // Step 5: Report results
    println!("\n{}", "=".repeat(80));
    println!("UNUSED VERUS SPECS ANALYSIS");
    println!("{}", "=".repeat(80));
    
    if unused_specs.is_empty() {
        println!("\n✓ No unused specs detected!");
    } else {
        println!("\n⚠ Found {} potentially unused specs:\n", unused_specs.len());
        
        let mut sorted_specs: Vec<_> = unused_specs.iter().collect();
        sorted_specs.sort_by(|a, b| a.identifier.cmp(&b.identifier));
        
        for (idx, spec) in sorted_specs.iter().enumerate() {
            println!("{}. {}", idx + 1, spec.identifier);
            if !spec.display_name.is_empty() {
                println!("   Display name: {}", spec.display_name);
            }
            if !spec.file_name.is_empty() {
                println!("   File: {}", spec.file_name);
            }
            if !spec.relative_path.is_empty() {
                println!("   Path: {}", spec.relative_path);
            }
            
            // Show first line of body if available
            if !spec.body.is_empty() {
                let first_line = spec.body.lines().next().unwrap_or("");
                if first_line.len() > 80 {
                    println!("   Decl: {}...", &first_line[..77]);
                } else {
                    println!("   Decl: {}", first_line);
                }
            }
            println!();
        }
    }

    println!("{}", "=".repeat(80));
    println!("IMPORTANT NOTES:");
    println!("{}", "=".repeat(80));
    println!("This analysis may produce FALSE POSITIVES for:");
    println!("  • Public API specs (intended for external use)");
    println!("  • Top-level theorems/lemmas (entry points)");
    println!("  • Specs used only in proof contexts");
    println!("  • Specs referenced through macros");
    println!("  • Test-only specifications");
    println!("\n⚠ Manual review is recommended before removing any specs!");
    println!("{}", "=".repeat(80));

    // Save results to a JSON file
    let results_file = format!("{}_unused_specs.json", 
        Path::new(project_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("project")
    );
    
    save_results_to_json(&unused_specs, &results_file)?;
    println!("\nResults saved to: {}", results_file);

    Ok(())
}

/// Find unused specs from atoms JSON
fn find_unused_specs(atoms_json_path: &str) -> Result<Vec<UnusedSpec>, Box<dyn std::error::Error>> {
    let contents = fs::read_to_string(atoms_json_path)?;
    let atoms: Vec<Atom> = serde_json::from_str(&contents)?;

    // Build a map: identifier -> Atom
    let atom_map: HashMap<String, &Atom> = atoms
        .iter()
        .map(|atom| (atom.identifier.clone(), atom))
        .collect();

    // Identify all spec identifiers
    let spec_identifiers: HashSet<String> = atoms
        .iter()
        .filter(|atom| is_spec_function(atom))
        .map(|atom| atom.identifier.clone())
        .collect();

    println!("Found {} total specs", spec_identifiers.len());

    // Collect all dependencies (union of all deps)
    let all_deps: HashSet<String> = atoms
        .iter()
        .flat_map(|atom| atom.deps.iter().cloned())
        .collect();

    println!("Found {} unique dependencies across all functions", all_deps.len());

    // Find specs that are NOT in the deps set
    let unused_spec_ids: HashSet<String> = spec_identifiers
        .difference(&all_deps)
        .cloned()
        .collect();

    // Build detailed info about unused specs
    let unused_specs: Vec<UnusedSpec> = unused_spec_ids
        .iter()
        .filter_map(|id| {
            atom_map.get(id).map(|atom| UnusedSpec {
                identifier: atom.identifier.clone(),
                display_name: atom.display_name.clone(),
                file_name: atom.file_name.clone(),
                relative_path: atom.relative_path.clone(),
                full_path: atom.full_path.clone(),
                body: atom.body.clone(),
                visibility: extract_visibility(&atom.body),
            })
        })
        .collect();

    Ok(unused_specs)
}

/// Check if an Atom represents a spec function
fn is_spec_function(atom: &Atom) -> bool {
    let body_lower = atom.body.to_lowercase();
    
    // Check for various spec patterns in the body
    body_lower.contains("spec fn") 
        || body_lower.contains("spec(") 
        || body_lower.starts_with("spec ")
        || (body_lower.contains("#[verifier") && body_lower.contains("spec"))
}

/// Extract visibility from function body (pub, pub(crate), private)
fn extract_visibility(body: &str) -> String {
    let body_trimmed = body.trim();
    
    if body_trimmed.starts_with("pub(crate)") {
        "pub(crate)".to_string()
    } else if body_trimmed.starts_with("pub open") {
        "pub open".to_string()
    } else if body_trimmed.starts_with("pub closed") {
        "pub closed".to_string()
    } else if body_trimmed.starts_with("pub") {
        "pub".to_string()
    } else {
        "private".to_string()
    }
}

/// Save results to a JSON file
fn save_results_to_json(
    unused_specs: &[UnusedSpec],
    output_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use serde::Serialize;
    
    #[derive(Serialize)]
    struct UnusedSpecsReport {
        summary: Summary,
        unused_specs: Vec<UnusedSpecOutput>,
        warnings: Vec<String>,
    }
    
    #[derive(Serialize)]
    struct Summary {
        total_unused_specs: usize,
        by_visibility: VisibilityBreakdown,
    }
    
    #[derive(Serialize)]
    struct VisibilityBreakdown {
        public: usize,
        pub_crate: usize,
        pub_open: usize,
        pub_closed: usize,
        private: usize,
    }
    
    #[derive(Serialize)]
    struct UnusedSpecOutput {
        identifier: String,
        display_name: String,
        visibility: String,
        file_name: String,
        relative_path: String,
        full_path: String,
        declaration: String,
    }
    
    // Count by visibility
    let mut vis_breakdown = VisibilityBreakdown {
        public: 0,
        pub_crate: 0,
        pub_open: 0,
        pub_closed: 0,
        private: 0,
    };
    
    for spec in unused_specs {
        match spec.visibility.as_str() {
            "pub" => vis_breakdown.public += 1,
            "pub(crate)" => vis_breakdown.pub_crate += 1,
            "pub open" => vis_breakdown.pub_open += 1,
            "pub closed" => vis_breakdown.pub_closed += 1,
            "private" => vis_breakdown.private += 1,
            _ => vis_breakdown.private += 1,
        }
    }
    
    // Convert to output format
    let mut sorted_specs: Vec<_> = unused_specs.iter().collect();
    sorted_specs.sort_by_key(|s| &s.identifier);
    
    let output_specs: Vec<UnusedSpecOutput> = sorted_specs
        .iter()
        .map(|spec| {
            let declaration = spec.body.lines().next().unwrap_or("").to_string();
            UnusedSpecOutput {
                identifier: spec.identifier.clone(),
                display_name: spec.display_name.clone(),
                visibility: spec.visibility.clone(),
                file_name: spec.file_name.clone(),
                relative_path: spec.relative_path.clone(),
                full_path: spec.full_path.clone(),
                declaration,
            }
        })
        .collect();
    
    let report = UnusedSpecsReport {
        summary: Summary {
            total_unused_specs: unused_specs.len(),
            by_visibility: vis_breakdown,
        },
        unused_specs: output_specs,
        warnings: vec![
            "This analysis may produce FALSE POSITIVES for:".to_string(),
            "• Public API specs (intended for external use)".to_string(),
            "• Top-level theorems/lemmas (entry points)".to_string(),
            "• Specs used only in proof contexts".to_string(),
            "• Specs referenced through macros".to_string(),
            "• Test-only specifications".to_string(),
            "".to_string(),
            "⚠ Manual review is recommended before removing any specs!".to_string(),
        ],
    };
    
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, json)?;
    
    Ok(())
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct UnusedSpec {
    identifier: String,
    display_name: String,
    file_name: String,
    relative_path: String,
    full_path: String,
    body: String,
    visibility: String,
}

