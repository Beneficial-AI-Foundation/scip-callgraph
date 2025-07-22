// filepath: /home/lacra/git_repos/rust-analyzer-test/src/bin/generate_file_subgraph_dot.rs
use rust_analyzer_test::scip_to_call_graph_json::{parse_scip_json, build_call_graph, generate_file_subgraph_dot};
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 {
        eprintln!("Usage: {} <input-scip-json> <file-path> <output-dot-file>", args[0]);
        std::process::exit(1);
    }

    let input_path = &args[1];
    let file_path = &args[2];
    let output_path = &args[3];

    println!("Parsing SCIP JSON from {}...", input_path);
    let scip_data = parse_scip_json(input_path)?;
    
    println!("Building call graph...");
    let call_graph = build_call_graph(&scip_data);
    println!("Call graph contains {} functions", call_graph.len());
    
    println!("Generating file subgraph DOT file for {} at {}...", file_path, output_path);
    match generate_file_subgraph_dot(&call_graph, file_path, output_path) {
        Ok(_) => {
            println!("File subgraph DOT file generated successfully!");
            println!("To generate SVG, run: dot -Tsvg {} -o file_subgraph.svg", output_path);
        },
        Err(e) => {
            eprintln!("Failed to generate file subgraph: {}", e);
            println!("Make sure the file path '{}' exists in the call graph", file_path);
            
            // Optionally list some available file paths to help the user
            println!("\nAvailable file paths in the call graph:");
            let mut file_paths: Vec<_> = call_graph.values()
                .map(|node| &node.file_path)
                .collect();
            file_paths.sort();
            file_paths.dedup();
            for (i, path) in file_paths.iter().enumerate().take(10) {
                println!("  {}", path);
            }
            if file_paths.len() > 10 {
                println!("  ... and {} more", file_paths.len() - 10);
            }
            
            std::process::exit(1);
        }
    }

    Ok(())
}