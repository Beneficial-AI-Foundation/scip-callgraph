//! DOT/Graphviz export functionality for CLI visualization.
//!
//! This module provides functions to export call graphs as DOT files:
//! - `generate_call_graph_dot` - Full call graph as DOT
//! - `generate_file_subgraph_dot` - Subgraph for a specific file
//! - `generate_files_subgraph_dot` - Subgraph for multiple files
//! - `generate_function_subgraph_dot` - Subgraph starting from specific functions
//! - `generate_call_graph_svg` - Simple SVG visualization

use crate::types::FunctionNode;
use log::debug;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::Path;
use std::process::Command;

/// Helper function to generate both SVG and PNG files from a DOT file using Graphviz
pub fn generate_svg_and_png_from_dot(dot_path: &str) -> std::io::Result<()> {
    let svg_path = if let Some(stripped) = dot_path.strip_suffix(".dot") {
        format!("{stripped}.svg")
    } else {
        format!("{dot_path}.svg")
    };

    let png_path = if let Some(stripped) = dot_path.strip_suffix(".dot") {
        format!("{stripped}.png")
    } else {
        format!("{dot_path}.png")
    };

    // Generate SVG
    let svg_status = Command::new("dot")
        .args(["-Tsvg", dot_path, "-o", &svg_path])
        .status()?;
    if !svg_status.success() {
        return Err(std::io::Error::other(format!(
            "Failed to generate SVG: dot exited with {svg_status}"
        )));
    }

    // Generate PNG
    let png_status = Command::new("dot")
        .args(["-Tpng", dot_path, "-o", &png_path])
        .status()?;
    if !png_status.success() {
        return Err(std::io::Error::other(format!(
            "Failed to generate PNG: dot exited with {png_status}"
        )));
    }

    Ok(())
}

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

/// Generate a DOT format string for the call graph
///
/// This function returns the DOT content as a String, which is useful when you
/// want to print to stdout or process the content before writing.
pub fn generate_call_graph_dot_string(call_graph: &HashMap<String, FunctionNode>) -> String {
    let mut dot = String::from("digraph call_graph {\n");
    dot.push_str("  rankdir=LR;\n");
    dot.push_str("  node [shape=box, style=filled, fillcolor=lightblue, fontname=Helvetica];\n");
    dot.push_str("  edge [color=black];\n\n");

    // Filter out unwanted paths
    let skip_paths = [
        "libsignal/rust/protocol/benches",
        "libsignal/rust/protocol/tests",
        "libsignal/rust/protocol/examples",
    ];
    let filtered_nodes: Vec<&FunctionNode> = call_graph
        .values()
        .filter(|node| !skip_paths.iter().any(|p| node.file_path.contains(p)))
        .collect();

    // Group nodes by module/directory
    let mut module_groups: BTreeMap<String, Vec<&FunctionNode>> = BTreeMap::new();
    for node in &filtered_nodes {
        let path = std::path::Path::new(&node.file_path);
        let module = path
            .parent()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "root".to_string());
        module_groups.entry(module).or_default().push(*node);
    }

    for (cluster_id, (module, nodes)) in module_groups.iter().enumerate() {
        dot.push_str(&format!(
            "  subgraph cluster_{cluster_id} {{\n    label = \"{module}\";\n    style=filled;\n    color=lightgrey;\n    fontname=Helvetica;\n"
        ));
        for node in nodes {
            let label = node.display_name.clone();
            let tooltip = if let Some(body) = &node.body {
                let plain = body.replace(['\n', '\r'], " ").replace('"', "' ");
                if plain.len() > 200 {
                    let truncated = &plain[..200];
                    format!("{truncated}...")
                } else {
                    plain
                }
            } else {
                "".to_string()
            };
            let symbol = &node.symbol;
            dot.push_str(&format!(
                "    \"{symbol}\" [label=\"{label}\", tooltip=\"{tooltip}\"]\n"
            ));
        }
        dot.push_str("  }\n");
    }

    dot.push('\n');

    // Add edges
    let filtered_symbols: HashSet<_> = filtered_nodes.iter().map(|n| &n.symbol).collect();
    for node in &filtered_nodes {
        for callee in &node.callees {
            if filtered_symbols.contains(callee) {
                let symbol = &node.symbol;
                dot.push_str(&format!("  \"{symbol}\" -> \"{callee}\"\n"));
            }
        }
    }

    dot.push_str("}\n");
    dot
}

/// Generate a DOT file format for the call graph that can be rendered by Graphviz
///
/// This writes the DOT file and also generates SVG and PNG files using Graphviz.
pub fn generate_call_graph_dot(
    call_graph: &HashMap<String, FunctionNode>,
    output_path: &str,
) -> std::io::Result<()> {
    let dot = generate_call_graph_dot_string(call_graph);
    std::fs::write(output_path, &dot)?;
    generate_svg_and_png_from_dot(output_path)?;
    Ok(())
}

/// Generate a DOT file for a subgraph containing only nodes from a specific file path
pub fn generate_file_subgraph_dot(
    call_graph: &HashMap<String, FunctionNode>,
    file_path: &str,
    output_path: &str,
) -> std::io::Result<()> {
    let mut dot = String::from("digraph file_subgraph {\n");
    dot.push_str("  rankdir=LR;\n");
    dot.push_str("  node [shape=box, style=filled, fontname=Helvetica];\n");
    dot.push_str("  edge [color=black];\n\n");

    // Find nodes that belong to the specified file
    let file_nodes: Vec<&FunctionNode> = call_graph
        .values()
        .filter(|node| {
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
                message.push_str(&format!("  - {path}\n"));
            }
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                message,
            ));
        } else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("No functions found in file path: {file_path}"),
            ));
        }
    }

    let file_symbols: HashSet<_> = file_nodes.iter().map(|n| &n.symbol).collect();

    // Add nodes
    for node in &file_nodes {
        let label = &node.display_name;
        let is_external_caller = node.callers.iter().any(|c| !file_symbols.contains(c));
        let is_external_callee = node.callees.iter().any(|c| !file_symbols.contains(c));

        let fillcolor = if is_external_caller && is_external_callee {
            "lightyellow"
        } else if is_external_caller {
            "lightgreen"
        } else if is_external_callee {
            "lightcoral"
        } else {
            "lightblue"
        };

        let symbol = &node.symbol;
        dot.push_str(&format!(
            "  \"{symbol}\" [label=\"{label}\", fillcolor={fillcolor}]\n"
        ));
    }

    dot.push('\n');

    // Add edges
    for node in &file_nodes {
        for callee in &node.callees {
            if file_symbols.contains(callee) {
                let symbol = &node.symbol;
                dot.push_str(&format!("  \"{symbol}\" -> \"{callee}\"\n"));
            }
        }
    }

    dot.push_str("}\n");
    std::fs::write(output_path, &dot)?;
    generate_svg_and_png_from_dot(output_path)?;
    Ok(())
}

/// Generate a DOT file for a subgraph containing nodes from multiple files
pub fn generate_files_subgraph_dot(
    call_graph: &HashMap<String, FunctionNode>,
    file_paths: &[String],
    output_path: &str,
) -> std::io::Result<()> {
    let mut dot = String::from("digraph files_subgraph {\n");
    dot.push_str("  rankdir=LR;\n");
    dot.push_str("  node [shape=box, style=filled, fontname=Helvetica];\n");
    dot.push_str("  edge [color=black];\n\n");

    // Find nodes that belong to any of the specified files
    let file_nodes: Vec<&FunctionNode> = call_graph
        .values()
        .filter(|node| {
            file_paths.iter().any(|file_path| {
                let requested_filename = Path::new(file_path)
                    .file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or(file_path);
                node.file_path.ends_with(file_path)
                    || node.file_path == *file_path
                    || node.symbol.contains(file_path)
                    || node.file_path.contains(requested_filename)
            })
        })
        .collect();

    if file_nodes.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("No functions found in file paths: {file_paths:?}"),
        ));
    }

    let file_symbols: HashSet<_> = file_nodes.iter().map(|n| &n.symbol).collect();

    // Group by file
    let mut file_groups: BTreeMap<String, Vec<&FunctionNode>> = BTreeMap::new();
    for node in &file_nodes {
        file_groups
            .entry(node.file_path.clone())
            .or_default()
            .push(*node);
    }

    // Create clusters
    for (cluster_id, (file_path, nodes)) in file_groups.iter().enumerate() {
        let file_name = Path::new(file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        dot.push_str(&format!("  subgraph cluster_{cluster_id} {{\n"));
        dot.push_str(&format!("    label = \"{file_name}\";\n"));
        dot.push_str("    style=filled;\n");
        dot.push_str("    color=lightgrey;\n");
        dot.push_str("    fontname=Helvetica;\n");

        for node in nodes {
            let label = &node.display_name;
            let symbol = &node.symbol;
            dot.push_str(&format!(
                "    \"{symbol}\" [label=\"{label}\", fillcolor=lightblue]\n"
            ));
        }

        dot.push_str("  }\n");
    }

    dot.push('\n');

    // Add edges
    for node in &file_nodes {
        for callee in &node.callees {
            if file_symbols.contains(callee) {
                let symbol = &node.symbol;
                dot.push_str(&format!("  \"{symbol}\" -> \"{callee}\"\n"));
            }
        }
    }

    dot.push_str("}\n");
    std::fs::write(output_path, &dot)?;
    generate_svg_and_png_from_dot(output_path)?;
    Ok(())
}

/// Generate a DOT file for a subgraph starting from specific functions with transitive dependencies
pub fn generate_function_subgraph_dot(
    call_graph: &HashMap<String, FunctionNode>,
    function_names: &[String],
    output_path: &str,
    include_callees: bool,
    include_callers: bool,
    depth: Option<usize>,
    filter_non_libsignal_sources: bool,
) -> std::io::Result<()> {
    let mut dot = String::from("digraph function_subgraph {\n");
    dot.push_str("  rankdir=LR;\n");
    dot.push_str("  node [shape=box, style=filled, fontname=Helvetica];\n");
    dot.push_str("  edge [color=black];\n\n");

    // Find nodes that match the specified function names
    let mut matched_nodes = Vec::new();
    let mut matched_symbols = HashSet::new();

    for function_name in function_names {
        let matches: Vec<_> = call_graph
            .values()
            .filter(|node| {
                if node.symbol == *function_name {
                    return true;
                }
                let normalized_symbol_query = function_name.trim_end_matches('.');
                if node.symbol.trim_end_matches('.') == normalized_symbol_query {
                    return true;
                }
                if node.display_name == *function_name {
                    return true;
                }
                let normalized_name = function_name.trim_end_matches("()");
                if let Some(func_part) = node.symbol.rsplit('#').next() {
                    let clean_func = func_part.trim_end_matches('.').trim_end_matches("()");
                    if clean_func == normalized_name {
                        return true;
                    }
                }
                if function_name.contains('#') {
                    if let Some(symbol_suffix) = node.symbol.rsplit('/').next() {
                        let clean_suffix = symbol_suffix.trim_end_matches('.').trim_end_matches("()");
                        let clean_query = function_name.trim_end_matches('.').trim_end_matches("()");
                        if clean_suffix.contains(clean_query) || clean_suffix.ends_with(clean_query) {
                            return true;
                        }
                    }
                }
                if node.symbol.contains(&format!("#{}", normalized_name))
                    && (node.symbol.ends_with(&format!("#{}().", normalized_name))
                        || node.symbol.ends_with(&format!("#{}.", normalized_name))
                        || node.symbol.contains(&format!("#{}/", normalized_name)))
                {
                    return true;
                }
                false
            })
            .collect();

        for node in matches {
            matched_nodes.push(node);
            matched_symbols.insert(node.symbol.clone());
        }
    }

    if matched_nodes.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("No functions found matching the provided names: {function_names:?}"),
        ));
    }

    debug!(
        "Found {} functions matching the provided names",
        matched_nodes.len()
    );

    // Build the transitive closure of dependencies
    let mut included_symbols = matched_symbols.clone();
    let mut queue = VecDeque::new();

    for symbol in &matched_symbols {
        queue.push_back((symbol.clone(), 0));
    }

    while let Some((symbol, current_depth)) = queue.pop_front() {
        if let Some(node) = call_graph.get(&symbol) {
            if include_callees {
                let should_include_callees = match depth {
                    Some(max_depth) => current_depth < max_depth,
                    None => true,
                };

                if should_include_callees {
                    for callee in &node.callees {
                        if !included_symbols.contains(callee) {
                            included_symbols.insert(callee.clone());
                            queue.push_back((callee.clone(), current_depth + 1));
                        }
                    }
                }
            }

            if include_callers {
                let should_include_callers = match depth {
                    Some(max_depth) => current_depth < max_depth,
                    None => true,
                };

                if should_include_callers {
                    for caller in &node.callers {
                        if !included_symbols.contains(caller) {
                            included_symbols.insert(caller.clone());
                            queue.push_back((caller.clone(), current_depth + 1));
                        }
                    }
                }
            }
        }
    }

    // Filter for libsignal sources if requested
    let final_included_symbols = if filter_non_libsignal_sources && include_callers && !include_callees {
        let mut has_incoming_edge = HashSet::new();
        for symbol in &included_symbols {
            if let Some(node) = call_graph.get(symbol) {
                for callee in &node.callees {
                    if included_symbols.contains(callee) {
                        has_incoming_edge.insert(callee.clone());
                    }
                }
            }
        }

        let source_nodes: Vec<_> = included_symbols
            .iter()
            .filter(|symbol| !has_incoming_edge.contains(*symbol))
            .cloned()
            .collect();

        let libsignal_sources: Vec<_> = source_nodes
            .iter()
            .filter(|symbol| call_graph.get(*symbol).map(is_libsignal_node).unwrap_or(false))
            .cloned()
            .collect();

        let mut filtered_symbols = HashSet::new();
        for source in &libsignal_sources {
            let mut stack = vec![source.clone()];
            let mut visited = HashSet::new();

            while let Some(symbol) = stack.pop() {
                if visited.contains(&symbol) {
                    continue;
                }
                visited.insert(symbol.clone());
                filtered_symbols.insert(symbol.clone());

                if let Some(node) = call_graph.get(&symbol) {
                    for callee in &node.callees {
                        if included_symbols.contains(callee) && !visited.contains(callee) {
                            stack.push(callee.clone());
                        }
                    }
                }
            }
        }
        filtered_symbols
    } else {
        included_symbols
    };

    // Separate libsignal nodes from non-libsignal nodes
    let mut libsignal_symbols = HashSet::new();
    for symbol in &final_included_symbols {
        if call_graph.get(symbol).map(is_libsignal_node).unwrap_or(false) {
            libsignal_symbols.insert(symbol.clone());
        }
    }

    // Group nodes by file path
    let mut file_groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for symbol in &final_included_symbols {
        if let Some(node) = call_graph.get(symbol) {
            file_groups
                .entry(node.file_path.clone())
                .or_default()
                .push(symbol.clone());
        }
    }

    // Create clusters
    for (cluster_id, (file_path, symbols)) in file_groups.iter().enumerate() {
        let file_label = Path::new(file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        dot.push_str(&format!("  subgraph cluster_{cluster_id} {{\n"));
        dot.push_str(&format!("    label = \"{file_label}\";\n"));
        dot.push_str("    style=filled;\n");

        let is_libsignal_cluster = symbols
            .iter()
            .any(|s| call_graph.get(s).map(is_libsignal_node).unwrap_or(false));

        if is_libsignal_cluster {
            dot.push_str("    color=lightblue;\n");
        } else {
            dot.push_str("    color=lightgrey;\n");
            dot.push_str("    style=\"filled,dotted\";\n");
        }
        dot.push_str("    fontname=Helvetica;\n");

        for symbol in symbols {
            if let Some(node) = call_graph.get(symbol) {
                let label = &node.display_name;
                let tooltip = if let Some(body) = &node.body {
                    let plain = body.replace(['\n', '\r'], " ").replace('"', "' ");
                    if plain.len() > 200 {
                        format!("{}...", &plain[..200])
                    } else {
                        plain
                    }
                } else {
                    "".to_string()
                };

                let (fillcolor, style) = if matched_symbols.contains(symbol) {
                    if libsignal_symbols.contains(symbol) {
                        ("blue", "filled")
                    } else {
                        ("green", "filled,dotted")
                    }
                } else if libsignal_symbols.contains(symbol) {
                    ("white", "filled")
                } else {
                    ("lightgray", "filled,dotted")
                };

                dot.push_str(&format!(
                    "    \"{}\" [label=\"{}\", tooltip=\"{}\", fillcolor={}, style=\"{}\"]\n",
                    node.symbol, label, tooltip, fillcolor, style
                ));
            }
        }

        dot.push_str("  }\n");
    }

    dot.push('\n');

    // Draw edges
    for symbol in &final_included_symbols {
        if let Some(node) = call_graph.get(symbol) {
            for callee in &node.callees {
                if final_included_symbols.contains(callee) {
                    let caller_is_libsignal = libsignal_symbols.contains(symbol);
                    let callee_is_libsignal = libsignal_symbols.contains(callee);

                    let edge_style = if caller_is_libsignal && callee_is_libsignal {
                        "color=blue, style=dashed"
                    } else if caller_is_libsignal && !callee_is_libsignal {
                        "color=blue"
                    } else if !caller_is_libsignal && callee_is_libsignal {
                        "color=orange, style=dashed"
                    } else {
                        "color=black, style=dashed"
                    };

                    dot.push_str(&format!(
                        "  \"{}\" -> \"{}\" [{}]\n",
                        node.symbol, callee, edge_style
                    ));
                }
            }
        }
    }

    dot.push_str("}\n");

    let final_output_path = if let Some(d) = depth {
        if let Some(stripped) = output_path.strip_suffix(".dot") {
            format!("{stripped}_depth_{d}.dot")
        } else {
            format!("{output_path}_depth_{d}")
        }
    } else {
        output_path.to_string()
    };

    std::fs::write(&final_output_path, &dot)?;
    generate_svg_and_png_from_dot(&final_output_path)?;
    Ok(())
}

/// Generate a simple SVG visualization of the call graph
pub fn generate_call_graph_svg(
    call_graph: &HashMap<String, FunctionNode>,
    output_path: &str,
) -> std::io::Result<()> {
    let width = 1200;
    let height = 800;
    let mut svg = format!(
        r#"<svg xmlns='http://www.w3.org/2000/svg' width='{width}' height='{height}' style='background:#fff;font-family:sans-serif'>\n"#
    );

    let nodes: Vec<_> = call_graph.values().collect();

    // Initial positions based on hash
    let mut positions = HashMap::new();
    let mut rng = std::hash::DefaultHasher::new();
    for node in &nodes {
        use std::hash::{Hash, Hasher};
        node.symbol.hash(&mut rng);
        let x = (rng.finish() % 800) as f64 + 200.0;
        rng.write_u8(1);
        let y = (rng.finish() % 600) as f64 + 100.0;
        positions.insert(&node.symbol, (x, y));
    }

    // Arrow marker
    svg.push_str("<defs><marker id='arrow' markerWidth='10' markerHeight='10' refX='10' refY='5' orient='auto' markerUnits='strokeWidth'><path d='M0,0 L10,5 L0,10 z' fill='#888'/></marker></defs>\n");

    // Draw edges
    for node in call_graph.values() {
        let (x1, y1) = positions[&node.symbol];
        for callee in &node.callees {
            if let Some(&(x2, y2)) = positions.get(callee) {
                svg.push_str(&format!(
                    "<line x1='{x1}' y1='{y1}' x2='{x2}' y2='{y2}' stroke='#888' stroke-width='2' marker-end='url(#arrow)'/>\n"
                ));
            }
        }
    }

    // Draw nodes
    for node in call_graph.values() {
        let (x, y) = positions[&node.symbol];
        let label = &node.display_name;
        svg.push_str(&format!(
            "<circle cx='{x}' cy='{y}' r='30' fill='lightblue' stroke='#333'/>\n"
        ));
        svg.push_str(&format!(
            "<text x='{x}' y='{}' text-anchor='middle' font-size='10'>{label}</text>\n",
            y + 4.0
        ));
    }

    svg.push_str("</svg>");
    std::fs::write(output_path, svg)
}

