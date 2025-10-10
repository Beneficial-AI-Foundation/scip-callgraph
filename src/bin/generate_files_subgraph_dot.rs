// filepath: /home/lacra/git_repos/baif/scip-callgraph/src/bin/generate_files_subgraph_dot.rs
use scip_callgraph::scip_to_call_graph_json::{
    build_call_graph, generate_files_subgraph_dot, parse_scip_json,
};
use scip_callgraph::logging::{init_logger, should_enable_debug};
use std::env;
use log::{debug, info, error};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 {
        eprintln!(
            "Usage: {} <input-scip-json> <output-dot-file> <file-path1> [<file-path2> ...] [--debug|-d]",
            args[0]
        );
        eprintln!(
            "Example: {} scip_data.json output.dot src/file1.rs src/file2.rs",
            args[0]
        );
        std::process::exit(1);
    }

    // Initialize logger based on debug flag
    let debug = should_enable_debug(&args);
    init_logger(debug);

    let input_path = &args[1];
    let output_path = &args[2];
    let file_paths = &args[3..];

    debug!("Parsing SCIP JSON from {input_path}...");
    let scip_data = parse_scip_json(input_path)?;

    debug!("Building call graph...");
    let call_graph = build_call_graph(&scip_data);
    info!("Call graph contains {} functions", call_graph.len());

    debug!(
        "Generating subgraph DOT file for {} files at {}...",
        file_paths.len(),
        output_path
    );

    // Convert file paths to owned Strings
    let file_paths: Vec<String> = file_paths.iter().map(|s| s.to_string()).collect();

    match generate_files_subgraph_dot(&call_graph, &file_paths, output_path) {
        Ok(_) => {
            info!("Files subgraph DOT and SVG files generated successfully!");
        }
        Err(e) => {
            error!("Failed to generate files subgraph: {e}");
            std::process::exit(1);
        }
    }

    Ok(())
}
