use scip_callgraph::scip_to_call_graph_json::{
    build_call_graph, generate_function_subgraph_dot, parse_scip_json,
};
use scip_callgraph::logging::{init_logger, should_enable_debug};
use std::env;
use log::{debug, info, error};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <input-scip-json> <function-name1> [<function-name2> ...] [--filter-non-libsignal-sources] [--include-callees] [--include-callers] [--depth <n>] [--debug|-d]", args[0]);
        eprintln!("Example: {} scip_data.json process_data handle_request --include-callers --depth 3", args[0]);
        eprintln!("Note: Output files will be automatically named based on function names and depth");
        std::process::exit(1);
    }

    // Initialize logger based on debug flag
    let debug = should_enable_debug(&args);
    init_logger(debug);

    let input_path = &args[1];

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
                    error!("Error: --depth must be followed by a valid number");
                    std::process::exit(1);
                }
            }
        } else {
            error!("Error: --depth must be followed by a number");
            std::process::exit(1);
        }
    } else {
        None
    };

    // Get function names (exclude flags) - now starting from args[2]
    let args_slice = &args[2..];
    let mut function_names = Vec::new();
    let mut skip_next = false;
    
    for arg in args_slice {
        if skip_next {
            skip_next = false;
            continue;
        }
        
        // Skip all flags
        if arg == "--include-callers" 
            || arg == "--include-callees"
            || arg == "--filter-non-libsignal-sources"
            || arg == "--debug"
            || arg == "-d" {
            continue;
        } else if arg == "--depth" {
            // Skip --depth and mark to skip the next argument (the number)
            skip_next = true;
            continue;
        }
        
        // This is a function name
        function_names.push(arg.to_string());
    }

    if function_names.is_empty() {
        error!("Error: At least one function name must be specified");
        std::process::exit(1);
    }
    
    // Generate output filename from function names and depth
    // Sanitize function names for use in filename
    let sanitized_names: Vec<String> = function_names
        .iter()
        .map(|name| {
            name.replace(' ', "_")
                .replace('/', "_")
                .replace('\\', "_")
                .replace('#', "_")
                .replace('(', "")
                .replace(')', "")
                .replace('.', "_")
        })
        .collect();
    
    // Use first function name or combine multiple with underscores (limit to 3 for readability)
    let base_name = if sanitized_names.len() == 1 {
        sanitized_names[0].clone()
    } else if sanitized_names.len() <= 3 {
        sanitized_names.join("_and_")
    } else {
        format!("{}_and_{}_others", sanitized_names[0], sanitized_names.len() - 1)
    };
    
    // The output_path will be modified in generate_function_subgraph_dot to include depth
    let output_path = format!("{}.dot", base_name);

    debug!("Parsing SCIP JSON from {input_path}...");
    let scip_data = parse_scip_json(input_path)?;

    debug!("Building call graph...");
    let call_graph = build_call_graph(&scip_data);
    info!("Call graph contains {} functions", call_graph.len());

    // Show what the actual output filename will be
    let final_filename = if let Some(d) = depth {
        let stripped = output_path.strip_suffix(".dot").unwrap_or(&output_path);
        format!("{stripped}_depth_{d}.dot")
    } else {
        output_path.clone()
    };
    
    debug!(
        "Generating function subgraph DOT file for {} functions as {}...",
        function_names.len(),
        final_filename
    );
    debug!("Include callees: {include_callees}");
    debug!("Include callers: {include_callers}");
    if let Some(d) = depth {
        debug!("Depth limit: {d}");
    } else {
        debug!("No depth limit");
    }

    match generate_function_subgraph_dot(
        &call_graph,
        &function_names,
        &output_path,
        include_callees,
        include_callers,
        depth,
        filter_non_libsignal_sources,
    ) {
        Ok(_) => {
            // Show the actual filenames that were created
            let svg_name = if let Some(stripped) = final_filename.strip_suffix(".dot") {
                format!("{stripped}.svg")
            } else {
                format!("{final_filename}.svg")
            };
            info!("✓ Generated files:");
            info!("  • {final_filename}");
            info!("  • {svg_name}");
        }
        Err(e) => {
            error!("Failed to generate function subgraph: {e}");
            std::process::exit(1);
        }
    }

    Ok(())
}
