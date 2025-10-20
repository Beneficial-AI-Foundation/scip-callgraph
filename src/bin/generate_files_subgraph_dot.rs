use scip_callgraph::scip_to_call_graph_json::{
    build_call_graph, generate_files_subgraph_dot, parse_scip_json,
};
use scip_callgraph::logging::init_logger;
use clap::Parser;
use log::{debug, info, error};

/// Generate files subgraph DOT files from SCIP data
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Input SCIP JSON file
    input_scip_json: String,

    /// Output DOT file path
    output_dot_file: String,

    /// File paths to include in the subgraph
    #[arg(required = true)]
    file_paths: Vec<String>,

    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Initialize logger based on debug flag
    init_logger(args.debug);

    debug!("Parsing SCIP JSON from {}...", args.input_scip_json);
    let scip_data = parse_scip_json(&args.input_scip_json)?;

    debug!("Building call graph...");
    let call_graph = build_call_graph(&scip_data);
    info!("Call graph contains {} functions", call_graph.len());

    debug!(
        "Generating subgraph DOT file for {} files at {}...",
        args.file_paths.len(),
        args.output_dot_file
    );

    match generate_files_subgraph_dot(&call_graph, &args.file_paths, &args.output_dot_file) {
        Ok(_) => {
            // Show the actual filenames that were created
            let svg_name = if let Some(stripped) = args.output_dot_file.strip_suffix(".dot") {
                format!("{stripped}.svg")
            } else {
                format!("{}.svg", args.output_dot_file)
            };
            info!("✓ Generated files:");
            info!("  • {}", args.output_dot_file);
            info!("  • {svg_name}");
        }
        Err(e) => {
            error!("Failed to generate files subgraph: {e}");
            std::process::exit(1);
        }
    }

    Ok(())
}
