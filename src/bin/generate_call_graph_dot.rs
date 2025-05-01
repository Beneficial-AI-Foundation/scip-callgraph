use rust_analyzer_test::scip_to_call_graph_json::{parse_scip_json, build_call_graph, generate_call_graph_dot};
use std::env;
use std::process::Command;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {

    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <path-to-folder> <output-dot-file>", args[0]);
        std::process::exit(1);
    }

    let folder_path = &args[1];
    let output_path = &args[2];
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
    
    println!("Generating DOT file at {}...", output_path);
    generate_call_graph_dot(&call_graph, output_path)?;
    println!("DOT file generated successfully!");
    println!("To generate SVG, run: dot -Tsvg {} -o graph.svg", output_path);

    Ok(())
}