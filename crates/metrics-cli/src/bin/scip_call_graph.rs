use clap::{Parser, Subcommand};
use scip_core::scip_call_graph;
use std::fs::File;
use std::io::Write;

/// SCIP Call Graph Generator
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Generate a call graph from SCIP JSON data
    Generate {
        /// Input SCIP JSON file path
        scip_json_file: String,
        /// Output DOT file path (optional, prints to stdout if not provided)
        output_dot_file: Option<String>,
    },
    /// Generate a filtered call graph starting from a specific function
    Filter {
        /// Input SCIP JSON file path
        scip_json_file: String,
        /// Function name to start filtering from
        function_name: String,
        /// Output DOT file path (optional, prints to stdout if not provided)
        output_dot_file: Option<String>,
        /// Maximum depth for traversal
        max_depth: Option<usize>,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    match args.command {
        Commands::Generate {
            scip_json_file,
            output_dot_file,
        } => {
            // Parse SCIP JSON data
            let scip_data = scip_call_graph::parse_scip_json(&scip_json_file)?;

            // Build the call graph
            let call_graph = scip_call_graph::build_call_graph(&scip_data);

            // Print summary
            println!("Call graph generated from {scip_json_file}");
            scip_call_graph::print_call_graph_summary(&call_graph);

            // Generate DOT file
            let dot_content = scip_call_graph::generate_call_graph_dot(&call_graph);

            if let Some(path) = output_dot_file {
                let mut file = File::create(&path)?;
                file.write_all(dot_content.as_bytes())?;
                println!("\nDOT file written to: {path}");
                println!("You can visualize it with: dot -Tpng {path} -o call_graph.png");
            } else {
                println!("\nDOT format call graph:\n");
                println!("{dot_content}");
            }
        }
        Commands::Filter {
            scip_json_file,
            function_name,
            output_dot_file,
            max_depth,
        } => {
            // Parse SCIP JSON data
            let scip_data = scip_call_graph::parse_scip_json(&scip_json_file)?;

            // Build the full call graph
            let full_graph = scip_call_graph::build_call_graph(&scip_data);

            // Find possible matches for the entry point
            let mut matching_entries = Vec::new();
            for (symbol, node) in &full_graph {
                if symbol.contains(&function_name) || node.display_name.contains(&function_name) {
                    matching_entries.push(symbol.clone());
                }
            }

            if matching_entries.is_empty() {
                println!("Error: No functions matching '{function_name}' found.");
                return Ok(());
            }

            // If we have multiple matches, show them and let user be more specific
            if matching_entries.len() > 1 {
                println!("Multiple functions match '{function_name}'. Please be more specific:");
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
                max_depth,
            );

            // Print summary
            println!("Filtered call graph starting from '{function_name}'");
            if let Some(depth) = max_depth {
                println!("(limited to depth {depth})");
            }
            scip_call_graph::print_call_graph_summary(&filtered_graph);

            // Generate DOT file
            let dot_content = scip_call_graph::generate_call_graph_dot(&filtered_graph);

            if let Some(path) = output_dot_file {
                let mut file = File::create(&path)?;
                file.write_all(dot_content.as_bytes())?;
                println!("\nFiltered DOT file written to: {path}");
                println!("You can visualize it with: dot -Tpng {path} -o filtered_call_graph.png");
            } else {
                println!("\nFiltered DOT format call graph:\n");
                println!("{dot_content}");
            }
        }
    }

    Ok(())
}
