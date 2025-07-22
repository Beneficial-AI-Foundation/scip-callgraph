use rust_analyzer_test::scip_to_call_graph_json::{parse_scip_json, build_call_graph, generate_call_graph_dot};
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <input-scip-json> <output-dot-file>", args[0]);
        std::process::exit(1);
    }

    let input_path = &args[1];
    let output_path = &args[2];

    println!("Parsing SCIP JSON from {}...", input_path);
    let scip_data = parse_scip_json(input_path)?;
    
    println!("Building call graph...");
    let call_graph = build_call_graph(&scip_data);
    println!("Call graph contains {} functions", call_graph.len());
    
    println!("Generating DOT file at {}...", output_path);
    generate_call_graph_dot(&call_graph, output_path)?;
    println!("DOT file generated successfully!");
    println!("To generate SVG, run: dot -Tsvg {} -o graph.svg", output_path);

    Ok(())
}