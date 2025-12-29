use clap::Parser;
use log::{debug, info};
use scip_core::logging::init_logger;
use scip_core::{build_call_graph, generate_call_graph_dot, parse_scip_json};

/// Generate call graph DOT files from SCIP data
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Input SCIP JSON file
    input_scip_json: String,

    /// Output DOT file path
    output_dot_file: String,

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

    debug!("Generating DOT file at {}...", args.output_dot_file);
    generate_call_graph_dot(&call_graph, &args.output_dot_file)?;

    // Show the actual filenames that were created
    let svg_name = if let Some(stripped) = args.output_dot_file.strip_suffix(".dot") {
        format!("{stripped}.svg")
    } else {
        format!("{}.svg", args.output_dot_file)
    };
    let png_name = if let Some(stripped) = args.output_dot_file.strip_suffix(".dot") {
        format!("{stripped}.png")
    } else {
        format!("{}.png", args.output_dot_file)
    };
    info!("✓ Generated files:");
    info!("  • {}", args.output_dot_file);
    info!("  • {svg_name}");
    info!("  • {png_name}");

    Ok(())
}
