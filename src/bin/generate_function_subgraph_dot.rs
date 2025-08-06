use rust_analyzer_test::scip_to_call_graph_json::{
    build_call_graph, generate_function_subgraph_dot, parse_scip_json,
};
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 {
        eprintln!("Usage: {} <input-scip-json> <output-dot-file> <function-name1> [<function-name2> ...] [--filter-non-libsignal-sources] [--include-callees] [--include-callers] [--depth <n>]", args[0]);
        eprintln!("Example: {} scip_data.json output.dot process_data handle_request --include-callers --depth 3", args[0]);
        std::process::exit(1);
    }

    let input_path = &args[1];
    let output_path = &args[2];

    let filter_non_libsignal_sources = args
        .iter()
        .any(|arg| arg == "--filter-non-libsignal-sources");

    // Check if --include-callees flag is present
    let include_callees = args.iter().any(|arg| arg == "--include-callees");

    // Check if --include-callers flag is present
    let include_callers = args.iter().any(|arg| arg == "--include-callers");

    // Parse --depth argument if present
    let depth = if let Some(depth_pos) = args.iter().position(|arg| arg == "--depth") {
        if depth_pos + 1 < args.len() {
            match args[depth_pos + 1].parse::<usize>() {
                Ok(d) => Some(d),
                Err(_) => {
                    eprintln!("Error: --depth must be followed by a valid number");
                    std::process::exit(1);
                }
            }
        } else {
            eprintln!("Error: --depth must be followed by a number");
            std::process::exit(1);
        }
    } else {
        None
    };

    // Get function names (exclude flags)
    let function_names: Vec<String> = args[3..]
        .iter()
        .enumerate()
        .filter(|(i, arg)| {
            // Skip --include-callers, --depth, and the number following --depth
            if *arg == "--include-callers" || *arg == "--depth" {
                false
            } else if *i > 0 && args[3 + i - 1] == "--depth" {
                // Skip the number that follows --depth
                false
            } else {
                true
            }
        })
        .map(|(_, s)| s.to_string())
        .collect();

    if function_names.is_empty() {
        eprintln!("Error: At least one function name must be specified");
        std::process::exit(1);
    }

    println!("Parsing SCIP JSON from {}...", input_path);
    let scip_data = parse_scip_json(input_path)?;

    println!("Building call graph...");
    let call_graph = build_call_graph(&scip_data);
    println!("Call graph contains {} functions", call_graph.len());

    println!(
        "Generating function subgraph DOT file for {} functions at {}...",
        function_names.len(),
        output_path
    );
    println!("Include callers: {}", include_callers);
    if let Some(d) = depth {
        println!("Depth limit for callers: {}", d);
    } else {
        println!("No depth limit for callers");
    }

    match generate_function_subgraph_dot(
        &call_graph,
        &function_names,
        output_path,
        include_callees,
        include_callers,
        depth,
        filter_non_libsignal_sources,
    ) {
        Ok(_) => {
            println!("Function subgraph DOT and SVG files generated successfully!");
        }
        Err(e) => {
            eprintln!("Failed to generate function subgraph: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}
