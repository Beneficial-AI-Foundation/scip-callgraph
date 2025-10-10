use scip_callgraph::scip_to_call_graph_json::{
    build_call_graph, generate_call_graph_dot, parse_scip_json,
};
use scip_callgraph::logging::{init_logger, should_enable_debug};
use std::env;
use log::{debug, info};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <input-scip-json> <output-dot-file> [--debug|-d]", args[0]);
        std::process::exit(1);
    }

    // Initialize logger based on debug flag
    let debug = should_enable_debug(&args);
    init_logger(debug);

    let input_path = &args[1];
    let output_path = &args[2];

    debug!("Parsing SCIP JSON from {input_path}...");
    let scip_data = parse_scip_json(input_path)?;

    debug!("Building call graph...");
    let call_graph = build_call_graph(&scip_data);
    info!("Call graph contains {} functions", call_graph.len());

    debug!("Generating DOT file at {output_path}...");
    generate_call_graph_dot(&call_graph, output_path)?;
    info!("DOT and SVG files generated successfully!");

    Ok(())
}
