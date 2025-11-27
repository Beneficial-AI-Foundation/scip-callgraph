use clap::Parser;
use log::{info, error};
use scip_core::logging::init_logger;
use scip_core::scip_to_call_graph_json::{
    build_call_graph, export_call_graph_d3, parse_scip_json,
};

/// Export call graph in D3.js force-directed graph format
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Input SCIP JSON file
    input_scip_json: String,

    /// Output JSON file for D3.js visualization
    #[arg(short, long, default_value = "call_graph_d3.json")]
    output: String,

    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Initialize logger based on debug flag
    init_logger(args.debug);

    info!("Parsing SCIP JSON from {}...", args.input_scip_json);
    let scip_data = parse_scip_json(&args.input_scip_json)?;

    info!("Building call graph...");
    let call_graph = build_call_graph(&scip_data);
    info!("Call graph contains {} functions", call_graph.len());

    info!("Exporting call graph to D3.js format...");
    match export_call_graph_d3(&call_graph, &scip_data, &args.output) {
        Ok(_) => {
            info!("✓ Successfully exported call graph to {}", args.output);
            info!("  Total nodes: {}", call_graph.len());
            
            // Count total edges
            let total_edges: usize = call_graph.values()
                .map(|node| node.callees.len())
                .sum();
            info!("  Total edges: {}", total_edges);
            
            // Count libsignal nodes
            let libsignal_count = call_graph.values()
                .filter(|node| {
                    node.symbol.contains("libsignal") || 
                    node.relative_path.contains("libsignal")
                })
                .count();
            info!("  Libsignal nodes: {}", libsignal_count);
            
            info!("\nNext steps:");
            info!("  1. Open the web viewer: open web/index.html");
            info!("  2. Load the exported file: {}", args.output);
        }
        Err(e) => {
            error!("Failed to export call graph: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}

