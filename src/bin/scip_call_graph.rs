use std::env;
use std::fs::File;
use std::io::Write;
use rust_analyzer_test::scip_call_graph;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        print_usage();
        return Ok(());
    }
    
    let command = &args[1];
    
    match command.as_str() {
        "generate" => {
            if args.len() < 3 {
                println!("Error: Missing SCIP JSON file path");
                print_usage();
                return Ok(());
            }
            let scip_path = &args[2];
            let output_path = if args.len() >= 4 {
                Some(&args[3])
            } else {
                None
            };
            
            // Parse SCIP JSON data
            let scip_data = scip_call_graph::parse_scip_json(scip_path)?;
            
            // Build the call graph
            let call_graph = scip_call_graph::build_call_graph(&scip_data);
            
            // Print summary
            println!("Call graph generated from {}", scip_path);
            scip_call_graph::print_call_graph_summary(&call_graph);
            
            // Generate DOT file
            let dot_content = scip_call_graph::generate_call_graph_dot(&call_graph);
            
            if let Some(path) = output_path {
                let mut file = File::create(path)?;
                file.write_all(dot_content.as_bytes())?;
                println!("\nDOT file written to: {}", path);
                println!("You can visualize it with: dot -Tpng {} -o call_graph.png", path);
            } else {
                println!("\nDOT format call graph:\n");
                println!("{}", dot_content);
            }
        },
        "filter" => {
            if args.len() < 4 {
                println!("Error: Missing required arguments");
                print_usage();
                return Ok(());
            }
            
            let scip_path = &args[2];
            let entry_point = &args[3];
            let output_path = if args.len() >= 5 {
                Some(&args[4])
            } else {
                None
            };
            
            let max_depth = if args.len() >= 6 {
                match args[5].parse::<usize>() {
                    Ok(depth) => Some(depth),
                    Err(_) => None
                }
            } else {
                None
            };
            
            // Parse SCIP JSON data
            let scip_data = scip_call_graph::parse_scip_json(scip_path)?;
            
            // Build the full call graph
            let full_graph = scip_call_graph::build_call_graph(&scip_data);
            
            // Find possible matches for the entry point
            let mut matching_entries = Vec::new();
            for (symbol, node) in &full_graph {
                if symbol.contains(entry_point) || node.display_name.contains(entry_point) {
                    matching_entries.push(symbol.clone());
                }
            }
            
            if matching_entries.is_empty() {
                println!("Error: No functions matching '{}' found.", entry_point);
                return Ok(());
            }
            
            // If we have multiple matches, show them and let user be more specific
            if matching_entries.len() > 1 {
                println!("Multiple functions match '{}'. Please be more specific:", entry_point);
                for (i, symbol) in matching_entries.iter().enumerate() {
                    if let Some(node) = full_graph.get(symbol) {
                        println!("  {}. {} ({})", i + 1, node.display_name, symbol);
                    }
                }
                return Ok(());
            }
            
            // Generate filtered graph
            let filtered_graph = scip_call_graph::generate_filtered_call_graph(
                &full_graph, 
                &[matching_entries[0].clone()], 
                max_depth
            );
            
            // Print summary
            println!("Filtered call graph starting from '{}'", entry_point);
            if let Some(depth) = max_depth {
                println!("(limited to depth {})", depth);
            }
            scip_call_graph::print_call_graph_summary(&filtered_graph);
            
            // Generate DOT file
            let dot_content = scip_call_graph::generate_call_graph_dot(&filtered_graph);
            
            if let Some(path) = output_path {
                let mut file = File::create(path)?;
                file.write_all(dot_content.as_bytes())?;
                println!("\nFiltered DOT file written to: {}", path);
                println!("You can visualize it with: dot -Tpng {} -o filtered_call_graph.png", path);
            } else {
                println!("\nFiltered DOT format call graph:\n");
                println!("{}", dot_content);
            }
        },
        "help" | _ => {
            print_usage();
        }
    }
    
    Ok(())
}

fn print_usage() {
    println!("SCIP Call Graph Generator");
    println!("\nUsage:");
    println!("  scip_call_graph <command> [arguments]");
    println!("\nCommands:");
    println!("  generate <scip_json_file> [output_dot_file]");
    println!("    Generate a call graph from SCIP JSON data");
    println!("\n  filter <scip_json_file> <function_name> [output_dot_file] [max_depth]");
    println!("    Generate a filtered call graph starting from a specific function");
    println!("\n  help");
    println!("    Show this help message");
    println!("\nExample:");
    println!("  scip_call_graph generate index_scip.json call_graph.dot");
    println!("  scip_call_graph filter index_scip.json main call_graph_main.dot 3");
}