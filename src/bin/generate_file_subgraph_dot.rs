// filepath: /home/lacra/git_repos/baif/scip-callgraph/src/bin/generate_file_subgraph_dot.rs
use scip_callgraph::scip_to_call_graph_json::{
    build_call_graph, parse_scip_json,
};
use scip_callgraph::logging::{init_logger, should_enable_debug};
use std::env;
use std::collections::{HashMap, HashSet};
use serde::{Deserialize, Serialize};
use log::{debug, info, warn, error};

#[derive(Debug, Deserialize, Serialize)]
struct VerificationReport {
    pub verification: VerificationResult,
}

#[derive(Debug, Deserialize, Serialize)]
struct VerificationResult {
    pub verified_functions: Vec<String>,
    pub failed_functions: Vec<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 {
        eprintln!(
            "Usage: {} <input-scip-json> <file-path> <output-dot-file> [verification-report.json] [--debug|-d]",
            args[0]
        );
        eprintln!(
            "Example: {} scip_data.json src/file.rs output.dot verification_report.json",
            args[0]
        );
        std::process::exit(1);
    }

    // Initialize logger based on debug flag
    let debug = should_enable_debug(&args);
    init_logger(debug);

    let input_path = &args[1];
    let file_path = &args[2];
    let output_path = &args[3];
    
    // Check if verification report is provided as 4th argument (before debug flags)
    let verification_report_path = if args.len() >= 5 && !args[4].starts_with("--") {
        Some(&args[4])
    } else {
        None
    };

    debug!("Parsing SCIP JSON from {input_path}...");
    let scip_data = parse_scip_json(input_path)?;

    debug!("Building call graph...");
    let call_graph = build_call_graph(&scip_data);
    info!("Call graph contains {} functions", call_graph.len());

    // Parse verification report if provided
    let verification_status = if let Some(report_path) = verification_report_path {
        debug!("Loading verification report from {report_path}...");
        match load_verification_report(report_path) {
            Ok(status) => {
                info!("Loaded verification report: {} verified, {} failed", 
                     status.verified_functions.len(), status.failed_functions.len());
                Some(status)
            }
            Err(e) => {
                warn!("Failed to load verification report: {e}");
                None
            }
        }
    } else {
        None
    };

    debug!(
        "Generating file subgraph DOT file for {file_path} at {output_path}..."
    );
    match generate_file_subgraph_dot_with_verification(&call_graph, file_path, output_path, &verification_status) {
        Ok(_) => {
            info!("File subgraph DOT and SVG files generated successfully!");
        }
        Err(e) => {
            error!("Failed to generate file subgraph: {e}");
            warn!(
                "Make sure the file path '{file_path}' exists in the call graph"
            );

            // Optionally list some available file paths to help the user
            debug!("Available file paths in the call graph:");
            let mut file_paths: Vec<_> = call_graph.values().map(|node| &node.file_path).collect();
            file_paths.sort();
            file_paths.dedup();
            for path in file_paths.iter().take(10) {
                debug!("  {path}");
            }
            if file_paths.len() > 10 {
                debug!("  ... and {} more", file_paths.len() - 10);
            }

            std::process::exit(1);
        }
    }

    Ok(())
}

fn load_verification_report(path: &str) -> Result<VerificationResult, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let report: VerificationReport = serde_json::from_str(&content)?;
    Ok(report.verification)
}

fn generate_file_subgraph_dot_with_verification(
    call_graph: &HashMap<String, scip_callgraph::scip_to_call_graph_json::FunctionNode>,
    file_path: &str,
    output_path: &str,
    verification_status: &Option<VerificationResult>,
) -> std::io::Result<()> {
    use std::path::Path;
    
    let mut dot = String::from("digraph file_subgraph {\n");
    dot.push_str("  rankdir=LR;\n");
    dot.push_str("  node [shape=box, style=filled, fontname=Helvetica];\n");
    dot.push_str("  edge [color=gray];\n\n");

    // Create verification lookup sets
    let verified_functions: HashSet<String> = verification_status
        .as_ref()
        .map(|v| v.verified_functions.iter().cloned().collect())
        .unwrap_or_default();
    
    let failed_functions: HashSet<String> = verification_status
        .as_ref()
        .map(|v| v.failed_functions.iter().cloned().collect())
        .unwrap_or_default();

    // Find nodes that belong to the specified file - more flexible path matching
    let file_nodes: Vec<&scip_callgraph::scip_to_call_graph_json::FunctionNode> = call_graph
        .values()
        .filter(|node| {
            // Extract the filename from the provided file_path argument
            let requested_filename = Path::new(file_path)
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or(file_path);
            node.file_path.ends_with(file_path)
                || node.file_path == file_path
                || node.symbol.contains(file_path)
                || node.file_path.contains(requested_filename)
        })
        .collect();

    if file_nodes.is_empty() {
        // List available paths that contain part of the requested path
        let matching_paths: HashSet<_> = call_graph
            .values()
            .filter(|node| node.file_path.contains(file_path))
            .map(|node| &node.file_path)
            .collect();

        if !matching_paths.is_empty() {
            let mut message = format!(
                "No exact match for file path: {file_path}\n\nHere are some similar paths:\n"
            );
            for path in matching_paths {
                message.push_str(&format!("  {path}\n"));
            }
            return Err(std::io::Error::new(std::io::ErrorKind::NotFound, message));
        }

        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("No functions found in file path: {file_path}"),
        ));
    }

    debug!("Found {} functions in file {}", file_nodes.len(), file_path);
    for node in &file_nodes {
        debug!("  - {} ({})", node.display_name, node.symbol);
    }

    // Get the symbols of nodes in the file
    let file_symbols: HashSet<String> = file_nodes.iter().map(|n| n.symbol.clone()).collect();

    // Nodes that are called by or call into nodes from this file (1st degree connections)
    let mut connected_symbols = HashSet::new();
    for node in &file_nodes {
        // Add callees
        for callee in &node.callees {
            connected_symbols.insert(callee.clone());
        }
        // Add callers
        for caller in &node.callers {
            connected_symbols.insert(caller.clone());
        }
    }

    // Draw file nodes with verification-based colors
    for node in &file_nodes {
        let label = node.display_name.clone();
        let tooltip = if let Some(body) = &node.body {
            let plain = body
                .replace(['\n', '\r'], " ")
                .replace('"', "' ");
            if plain.len() > 200 {
                format!("{}...", &plain[..200])
            } else {
                plain
            }
        } else {
            "".to_string()
        };

        // Determine color based on verification status
        let fillcolor = if verified_functions.contains(&node.display_name) {
            "lightgreen"  // Verified functions in light green
        } else if failed_functions.contains(&node.display_name) {
            "lightcoral"  // Failed functions in light red
        } else {
            "lightblue"   // Default color for functions not in verification report
        };

        dot.push_str(&format!(
            "  \"{}\" [label=\"{}\", tooltip=\"{}\", fillcolor={}]\n",
            node.symbol, label, tooltip, fillcolor
        ));
    }

    // Draw connected nodes with light gray background
    for symbol in &connected_symbols {
        if !file_symbols.contains(symbol) {
            if let Some(node) = call_graph.get(symbol) {
                let label = node.display_name.clone();
                
                // Determine color for connected nodes based on verification status
                let fillcolor = if verified_functions.contains(&node.display_name) {
                    "lightgreen"  // Verified functions in light green
                } else if failed_functions.contains(&node.display_name) {
                    "lightcoral"  // Failed functions in light red
                } else {
                    "lightgray"   // Default color for connected nodes
                };
                
                dot.push_str(&format!(
                    "  \"{}\" [label=\"{}\", fillcolor={}]\n",
                    node.symbol, label, fillcolor
                ));
            }
        }
    }

    dot.push('\n');

    // Draw edges from file nodes to their callees
    for node in &file_nodes {
        for callee in &node.callees {
            if file_symbols.contains(callee) || connected_symbols.contains(callee) {
                dot.push_str(&format!("  \"{}\" -> \"{}\"\n", node.symbol, callee));
            }
        }
    }

    // Draw edges from callers to file nodes
    for node in &file_nodes {
        for caller in &node.callers {
            if !file_symbols.contains(caller) && connected_symbols.contains(caller) {
                dot.push_str(&format!("  \"{}\" -> \"{}\"\n", caller, node.symbol));
            }
        }
    }

    dot.push_str("}\n");
    // Write the DOT file
    std::fs::write(output_path, &dot)?;
    // Generate SVG using Graphviz
    let svg_path = if let Some(stripped) = output_path.strip_suffix(".dot") {
        format!("{stripped}.svg")
    } else {
        format!("{output_path}.svg")
    };
    let status = std::process::Command::new("dot")
        .args(["-Tsvg", output_path, "-o", &svg_path])
        .status()?;
    if !status.success() {
        return Err(std::io::Error::other(
            format!("Failed to generate SVG: dot exited with {status}"),
        ));
    }
    Ok(())
}
