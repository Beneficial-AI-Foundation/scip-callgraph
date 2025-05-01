use rust_analyzer_test::scip_to_call_graph_json::{parse_scip_json, build_call_graph, generate_call_graph_dot,  write_call_graph_as_atoms_json};
use std::env;
use std::process::Command;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <path-to-folder>", args[0]);
        std::process::exit(1);
    }

    let folder_path = &args[1];
    let folder = Path::new(folder_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("output");
    let dot_output_path = format!("{}.dot", folder);
    let json_output_path = format!("{}.json", folder);
    let scip_file = format!("{}/index.scip", folder_path);
    let scip_json_file = format!("{}_scip.json", folder_path);

    // Run rust-analyzer scip <path_to_folder>
    println!("Running: rust-analyzer scip {}", folder_path);
    let status = Command::new("rust-analyzer")
        .arg("scip")
        .arg(folder_path)
        .status()?;
    if !status.success() {
        eprintln!("Failed to run rust-analyzer scip");
        std::process::exit(1);
    }

    // Run scip print --json > <folder>_scip.json
    println!("Running: scip print --json > {}", scip_json_file);
    let scip_print = Command::new("scip")
        .arg("print")
        .arg("--json")
        .arg(&scip_file)
        .output()?;
    if !scip_print.status.success() {
        eprintln!("Failed to run scip print");
        std::process::exit(1);
    }
    std::fs::write(&scip_json_file, &scip_print.stdout)?;

    println!("Parsing SCIP JSON from {}...", scip_json_file);
    let scip_data = parse_scip_json(&scip_json_file)?;
    
    println!("Building call graph...");
    let call_graph = build_call_graph(&scip_data);
    println!("Call graph contains {} functions", call_graph.len());
    
    println!("Generating DOT file at {}...", dot_output_path);
    generate_call_graph_dot(&call_graph, &dot_output_path)?;
    println!("DOT file generated successfully!");
    println!("To generate SVG, run: dot -Tsvg {} -o graph.svg", dot_output_path);
    

    if let Err(e) = write_call_graph_as_atoms_json(&call_graph, &json_output_path) {
        eprintln!("Failed to write atoms JSON: {}", e);
        std::process::exit(1);
    }
    println!("Atoms JSON written to {}", json_output_path);

    Ok(())
}