use clap::Parser;
use log::{debug, error, info};
use scip_core::logging::init_logger;
use scip_core::scip_to_call_graph_json::{
    build_call_graph, generate_function_subgraph_dot, parse_scip_json,
};

/// Generate function subgraph DOT files from SCIP data
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Input SCIP JSON file
    input_scip_json: String,

    /// Function names to include in the subgraph
    #[arg(required = true)]
    function_names: Vec<String>,

    /// Filter out non-libsignal sources
    #[arg(long)]
    filter_non_libsignal_sources: bool,

    /// Include callees in the subgraph
    #[arg(long)]
    include_callees: bool,

    /// Include callers in the subgraph
    #[arg(long)]
    include_callers: bool,

    /// Maximum depth for traversal
    #[arg(long)]
    depth: Option<usize>,

    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Initialize logger based on debug flag
    init_logger(args.debug);

    // Generate output filename from function names and depth
    // Sanitize function names for use in filename
    let sanitized_names: Vec<String> = args
        .function_names
        .iter()
        .map(|name| {
            name.replace([' ', '/', '\\', '#'], "_")
                .replace(['(', ')'], "")
                .replace('.', "_")
        })
        .collect();

    // Use first function name or combine multiple with underscores (limit to 3 for readability)
    let base_name = if sanitized_names.len() == 1 {
        sanitized_names[0].clone()
    } else if sanitized_names.len() <= 3 {
        sanitized_names.join("_and_")
    } else {
        format!(
            "{}_and_{}_others",
            sanitized_names[0],
            sanitized_names.len() - 1
        )
    };

    // Library function will add depth to filename automatically
    let output_dot_file = format!("{base_name}.dot");

    debug!("Parsing SCIP JSON from {}...", args.input_scip_json);
    let scip_data = parse_scip_json(&args.input_scip_json)?;

    debug!("Building call graph...");
    let call_graph = build_call_graph(&scip_data);
    info!("Call graph contains {} functions", call_graph.len());

    debug!(
        "Generating function subgraph DOT file for {} functions as {}...",
        args.function_names.len(),
        output_dot_file
    );
    debug!("Include callees: {}", args.include_callees);
    debug!("Include callers: {}", args.include_callers);
    if let Some(d) = args.depth {
        debug!("Depth limit: {d}");
    } else {
        debug!("No depth limit");
    }

    match generate_function_subgraph_dot(
        &call_graph,
        &args.function_names,
        &output_dot_file,
        args.include_callees,
        args.include_callers,
        args.depth,
        args.filter_non_libsignal_sources,
    ) {
        Ok(_) => {
            // Show the actual filenames that were created
            let svg_name = if let Some(stripped) = output_dot_file.strip_suffix(".dot") {
                format!("{stripped}.svg")
            } else {
                format!("{output_dot_file}.svg")
            };
            let png_name = if let Some(stripped) = output_dot_file.strip_suffix(".dot") {
                format!("{stripped}.png")
            } else {
                format!("{output_dot_file}.png")
            };
            info!("✓ Generated files:");
            info!("  • {output_dot_file}");
            info!("  • {svg_name}");
            info!("  • {png_name}");
        }
        Err(e) => {
            error!("Failed to generate function subgraph: {e}");
            std::process::exit(1);
        }
    }

    Ok(())
}
