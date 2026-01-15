//! D3.js and web export functionality.
//!
//! This module provides functions to export call graphs for web visualization:
//! - `export_call_graph_d3` - Export to D3.js force-directed graph format
//! - `write_call_graph_as_atoms_json` - Export as JSON array of Atom objects

use crate::call_graph::{detect_function_mode, symbol_to_path};
use crate::types::{
    Atom, D3Graph, D3GraphMetadata, D3Link, D3Node, FunctionMode, FunctionNode, ScipIndex,
};
use log::debug;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Helper function to determine if a node is from libsignal
fn is_libsignal_node(node: &FunctionNode) -> bool {
    node.symbol.contains("libsignal-protocol")
        || node.symbol.contains("libsignal-core")
        || node.symbol.contains("libsignal-net")
        || node.symbol.contains("libsignal-keytrans")
        || node.symbol.contains("libsignal-svrb")
        || node.symbol.contains("libsignal")
        || node.relative_path.contains("libsignal")
        || node.symbol.contains("zkgroup")
        || node.symbol.contains("poksho")
        || node.symbol.contains("zkcredential")
        || node.symbol.contains("usernames")
}

/// Write the call graph as a JSON array of Atom objects
pub fn write_call_graph_as_atoms_json<P: AsRef<std::path::Path>>(
    call_graph: &HashMap<String, FunctionNode>,
    output_path: P,
) -> std::io::Result<()> {
    let atoms: Vec<Atom> = call_graph
        .values()
        .map(|node| {
            let body_content = node.body.clone().unwrap_or_default();

            let display_name = &node.display_name;
            let body_len = body_content.len();
            debug!("Function: {display_name}, Body length: {body_len}");

            let parent_folder = Path::new(&node.file_path)
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|name| name.to_str())
                .unwrap_or("unknown")
                .to_string();

            Atom {
                identifier: symbol_to_path(&node.symbol, &node.display_name),
                statement_type: "function".to_string(),
                deps: node
                    .callees
                    .iter()
                    .filter_map(|callee| call_graph.get(callee))
                    .map(|callee_node| {
                        symbol_to_path(&callee_node.symbol, &callee_node.display_name)
                    })
                    .collect(),
                body: body_content,
                display_name: node.display_name.clone(),
                full_path: node.file_path.clone(),
                relative_path: node.relative_path.clone(),
                file_name: Path::new(&node.file_path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
                parent_folder,
            }
        })
        .collect();

    let json = serde_json::to_string_pretty(&atoms)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(output_path, json)
}

/// Export the call graph in D3.js force-directed graph format
pub fn export_call_graph_d3<P: AsRef<std::path::Path>>(
    call_graph: &HashMap<String, FunctionNode>,
    scip_data: &ScipIndex,
    output_path: P,
) -> std::io::Result<()> {
    // Create nodes
    let nodes: Vec<D3Node> = call_graph
        .values()
        .map(|node| {
            let is_external = node.file_path.starts_with("external:");

            let (file_name, parent_folder) = if is_external {
                let crate_name = node
                    .file_path
                    .strip_prefix("external:")
                    .and_then(|s| s.split_whitespace().nth(2))
                    .unwrap_or("external")
                    .to_string();
                (crate_name.clone(), crate_name)
            } else {
                let parent = Path::new(&node.file_path)
                    .parent()
                    .and_then(|p| p.file_name())
                    .and_then(|name| name.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                let file = Path::new(&node.file_path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                (file, parent)
            };

            // Extract line numbers from range (SCIP uses 0-based, convert to 1-based)
            let (start_line, end_line) = if node.range.len() >= 4 {
                (
                    Some(node.range[0] as usize + 1),
                    Some(node.range[2] as usize + 1),
                )
            } else if !node.range.is_empty() {
                (
                    Some(node.range[0] as usize + 1),
                    Some(node.range[0] as usize + 1),
                )
            } else {
                (None, None)
            };

            let mode = node
                .body
                .as_ref()
                .map(|b| detect_function_mode(b))
                .unwrap_or(FunctionMode::Exec);

            D3Node {
                id: node.symbol.clone(),
                display_name: node.display_name.clone(),
                symbol: node.symbol.clone(),
                full_path: node.file_path.clone(),
                relative_path: node.relative_path.clone(),
                file_name,
                parent_folder,
                start_line,
                end_line,
                is_libsignal: is_libsignal_node(node),
                dependencies: node.callees.iter().cloned().collect(),
                dependents: node.callers.iter().cloned().collect(),
                mode,
            }
        })
        .collect();

    // Create links from the callee occurrences (with call location classification)
    let mut link_set: HashSet<(String, String, String)> = HashSet::new();
    let mut links: Vec<D3Link> = Vec::new();

    for node in call_graph.values() {
        for occurrence in &node.callee_occurrences {
            if call_graph.contains_key(&occurrence.symbol) {
                let link_type = occurrence
                    .location
                    .as_ref()
                    .map(|loc| loc.as_str())
                    .unwrap_or("inner")
                    .to_string();

                let key = (
                    node.symbol.clone(),
                    occurrence.symbol.clone(),
                    link_type.clone(),
                );

                if link_set.insert(key) {
                    links.push(D3Link {
                        source: node.symbol.clone(),
                        target: occurrence.symbol.clone(),
                        link_type,
                    });
                }
            }
        }
    }

    // Generate timestamp
    let now = chrono::Utc::now();
    let timestamp = now.to_rfc3339();

    // Create metadata
    let metadata = D3GraphMetadata {
        total_nodes: nodes.len(),
        total_edges: links.len(),
        project_root: scip_data.metadata.project_root.clone(),
        generated_at: timestamp,
        github_url: None,
    };

    // Create the full graph structure
    let graph = D3Graph {
        nodes,
        links,
        metadata,
    };

    // Write to file
    let json = serde_json::to_string_pretty(&graph)?;
    std::fs::write(output_path, json)
}
