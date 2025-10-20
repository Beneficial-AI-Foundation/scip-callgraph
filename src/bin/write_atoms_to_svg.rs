use log::{error, info};
use scip_callgraph::logging::{init_logger, should_enable_debug};
use scip_callgraph::scip_to_call_graph_json::{
    build_call_graph, generate_call_graph_svg, parse_scip_json,
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!(
            "Usage: {} <input_scip_json> <output_atoms_json> [--debug|-d]",
            args[0]
        );
        std::process::exit(1);
    }

    // Initialize logger based on debug flag
    let debug = should_enable_debug(&args);
    init_logger(debug);

    let input_path = &args[1];
    let output_path = &args[2];

    let scip_index = match parse_scip_json(input_path) {
        Ok(idx) => idx,
        Err(e) => {
            error!("Failed to parse SCIP JSON: {e}");
            std::process::exit(1);
        }
    };
    let call_graph = build_call_graph(&scip_index);

    if let Err(e) = generate_call_graph_svg(&call_graph, output_path) {
        error!("Failed to write atoms to SVG: {e}");
        std::process::exit(1);
    }
    info!("Atoms SVG written to {output_path}");
}
