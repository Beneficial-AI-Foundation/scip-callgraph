use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use serde::{Deserialize, Serialize};
use serde_json;

// Re-using the SCIP data structures from our JSON parser
#[derive(Debug, Serialize, Deserialize)]
pub struct ScipIndex {
    pub metadata: Metadata,
    pub documents: Vec<Document>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Metadata {
    pub tool_info: ToolInfo,
    pub project_root: String,
    pub text_document_encoding: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Document {
    pub language: String,
    pub relative_path: String,
    pub occurrences: Vec<Occurrence>,
    pub symbols: Vec<Symbol>,
    pub position_encoding: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Occurrence {
    pub range: Vec<i32>,
    pub symbol: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_roles: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Symbol {
    pub symbol: String,
    pub kind: i32,
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation: Option<Vec<String>>,
    pub signature_documentation: SignatureDocumentation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enclosing_symbol: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignatureDocumentation {
    pub language: String,
    pub text: String,
    pub position_encoding: i32,
}

/// Represents a node in the call graph
#[derive(Debug, Clone)]
pub struct FunctionNode {
    pub symbol: String,
    pub display_name: String,
    pub file_path: String,
    pub callers: HashSet<String>,  // Symbols that call this function
    pub callees: HashSet<String>,  // Symbols that this function calls
    pub range: Vec<i32>,  // Range of the function in the source file
    pub body: Option<String>,  // Optional body of the function
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Atom {
    pub identifier: String,
    pub statement_type: String,
    pub deps: Vec<String>,
    pub body: String,
}

/// Parse a SCIP JSON file
pub fn parse_scip_json(file_path: &str) -> Result<ScipIndex, Box<dyn std::error::Error>> {
    let path = Path::new(file_path);
    let contents = fs::read_to_string(path)?;
    let index: ScipIndex = serde_json::from_str(&contents)?;
    Ok(index)
}

/// Build a call graph from SCIP JSON data
pub fn build_call_graph(scip_data: &ScipIndex) -> HashMap<String, FunctionNode> {
    let mut call_graph: HashMap<String, FunctionNode> = HashMap::new();
    let mut symbol_to_file: HashMap<String, String> = HashMap::new();
    let mut symbol_to_kind: HashMap<String, i32> = HashMap::new();
    let mut function_symbols: HashSet<String> = HashSet::new();

    let project_root = scip_data.metadata.project_root.trim_start_matches("file://");
    // First pass: identify all function symbols and their containing files
    for doc in &scip_data.documents {
        for symbol in &doc.symbols {
            // Check if this is a function-like symbol (kind 12, 17, 80 etc.)
            if is_function_like(symbol.kind) {
                function_symbols.insert(symbol.symbol.clone());
                let abs_file_path = format!("{}/{}", project_root.trim_end_matches('/'), doc.relative_path.trim_start_matches('/'));
                symbol_to_file.insert(symbol.symbol.clone(), abs_file_path.clone());
                symbol_to_kind.insert(symbol.symbol.clone(), symbol.kind);

                // Initialize node in the call graph
                call_graph.insert(symbol.symbol.clone(), FunctionNode {
                    symbol: symbol.symbol.clone(),
                    display_name: symbol.display_name.clone().unwrap_or_else(|| "unknown".to_string()),
                    file_path: abs_file_path,
                    callers: HashSet::new(),
                    callees: HashSet::new(),
                    range: Vec::new(),  // Will be filled in the second pass
                    body: None,         // Will be filled after ranges are set
                });
            }
        }
    }

    // Second pass: analyze occurrences to build the call graph
    for doc in &scip_data.documents {
        // Track the current function context we're in
        let mut current_function: Option<String> = None;

        // Sort occurrences by range to process them in order of appearance
        let mut ordered_occurrences = doc.occurrences.clone();
        ordered_occurrences.sort_by(|a, b| {
            let a_start = (a.range[0], a.range[1]);
            let b_start = (b.range[0], b.range[1]);
            a_start.cmp(&b_start)
        });

        for occurrence in &ordered_occurrences {
            let is_definition = occurrence.symbol_roles.unwrap_or(0) & 1 == 1;

            // If this is a function definition, update the current context
            if is_definition && function_symbols.contains(&occurrence.symbol) {
                current_function = Some(occurrence.symbol.clone());
                // Also update the range for this function node
                if let Some(node) = call_graph.get_mut(&occurrence.symbol) {
                    node.range = occurrence.range.clone();
                }
            }

            // If this is a function call and we're inside a function
            if !is_definition && function_symbols.contains(&occurrence.symbol) {
                if let Some(caller) = &current_function {
                    if caller != &occurrence.symbol {  // Avoid self-calls for recursion
                        // Update the caller's callees
                        if let Some(caller_node) = call_graph.get_mut(caller) {
                            caller_node.callees.insert(occurrence.symbol.clone());
                        }

                        // Update the callee's callers
                        if let Some(callee_node) = call_graph.get_mut(&occurrence.symbol) {
                            callee_node.callers.insert(caller.clone());
                        }
                    }
                }
            }
        }
    }

    // Third pass: extract function bodies from source files
    for node in call_graph.values_mut() {
        println!("Processing {}", node.display_name);
        if !node.range.is_empty() {
            let file_path = &node.file_path;
            let abs_path = Path::new(file_path);
            println!("Reading file: {:?} with range {:?}", abs_path, &node.range);
            if let Ok(contents) = fs::read_to_string(abs_path) {
                let lines: Vec<&str> = contents.lines().collect();
                if node.range.len() == 3 {
                    let start_line = node.range[0] as usize;
                    if start_line < lines.len() {
                        let mut body_lines = Vec::new();
                        let mut open_braces = 0;
                        let mut found_first_brace = false;
                        for line in &lines[start_line..] {
                            if !found_first_brace {
                                if let Some(pos) = line.find('{') {
                                    found_first_brace = true;
                                    open_braces += 1;
                                    body_lines.push(&line[pos..]);
                                    if pos > 0 {
                                        body_lines.insert(0, &line[..pos]);
                                    }
                                } else {
                                    body_lines.push(line);
                                }
                            } else {
                                open_braces += line.matches('{').count();
                                open_braces -= line.matches('}').count();
                                body_lines.push(line);
                                if open_braces == 0 {
                                    break;
                                }
                            }
                        }
                        node.body = Some(body_lines.join("\n"));
                    }
                }
            }
        }
    }
    call_graph
}

/// Write the call graph as a JSON array of Atom objects
pub fn write_call_graph_as_atoms_json<P: AsRef<std::path::Path>>(
    call_graph: &HashMap<String, FunctionNode>,
    output_path: P,
) -> std::io::Result<()> {
    let atoms: Vec<Atom> = call_graph.values().map(|node| Atom {
        identifier: node.symbol.clone(),
        statement_type: "function".to_string(),
        deps: node.callees.iter().cloned().collect(),
        body: node.body.clone().unwrap_or_default(),
    }).collect();
    let json = serde_json::to_string_pretty(&atoms).unwrap();
    std::fs::write(output_path, json)
}

/// Check if a symbol kind represents a function-like entity
fn is_function_like(kind: i32) -> bool {
    match kind {
        6 | 12 | 17 | 80 => true,  // Method, Function, etc.
        _ => false,
    }
}

/// Generate a DOT file format for the call graph that can be rendered by Graphviz
pub fn generate_call_graph_dot(call_graph: &HashMap<String, FunctionNode>, output_path: &str) -> std::io::Result<()> {
    use std::collections::BTreeMap;
    let mut dot = String::from("digraph call_graph {\n");
    dot.push_str("  node [shape=box, style=filled, fillcolor=lightblue];\n");
    dot.push_str("  edge [color=gray];\n\n");

    // Group nodes by file path
    let mut path_groups: BTreeMap<&str, Vec<&FunctionNode>> = BTreeMap::new();
    for node in call_graph.values() {
        path_groups.entry(&node.file_path[..]).or_default().push(node);
    }

    let mut cluster_id = 0;
    for (path, nodes) in &path_groups {
        dot.push_str(&format!("  subgraph cluster_{} {{\n    label = \"{}\";\n    style=filled;\n    color=lightgrey;\n", cluster_id, path));
        for node in nodes {
            let escaped_name = node.display_name.replace("\"", "\\\"");
            let escaped_path = node.file_path.replace("\"", "\\\"");
            let label = format!("{} ({})", escaped_name, escaped_path);
            let tooltip = if let Some(body) = &node.body {
                let plain = body.replace('\n', " ").replace('\r', " ").replace('"', "' ");
                if plain.len() > 200 {
                    format!("{}...", &plain[..200])
                } else {
                    plain
                }
            } else {
                "".to_string()
            };
            dot.push_str(&format!(
                "    \"{}\" [label=\"{}\", tooltip=\"{}\"]\n",
                node.symbol, label, tooltip
            ));
        }
        dot.push_str("  }\n");
        cluster_id += 1;
    }

    dot.push_str("\n");

    // Add edges
    for node in call_graph.values() {
        for callee in &node.callees {
            if call_graph.contains_key(callee) {
                dot.push_str(&format!("  \"{}\" -> \"{}\"\n", node.symbol, callee));
            }
        }
    }

    dot.push_str("}\n");
    std::fs::write(output_path, dot)
}

pub fn generate_call_graph_svg(call_graph: &HashMap<String, FunctionNode>, output_path: &str) -> std::io::Result<()> {
    let node_radius = 40;
    let width = 1200;
    let height = 800;
    let mut svg = format!(
        r#"<svg xmlns='http://www.w3.org/2000/svg' width='{w}' height='{h}' style='background:#fff;font-family:sans-serif'>\n"#,
        w=width, h=height
    );

    // Use a force-directed layout instead of circular
    // This is a simple implementation - for complex graphs, use a dot file with Graphviz
    let mut nodes: Vec<_> = call_graph.values().collect();
    let n = nodes.len();
    
    // Initial random positions
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

    // Arrows marker definition
    svg.push_str("<defs><marker id='arrow' markerWidth='10' markerHeight='10' refX='10' refY='5' orient='auto' markerUnits='strokeWidth'><path d='M0,0 L10,5 L0,10 z' fill='#888'/></marker></defs>\n");

    // Draw edges
    for node in call_graph.values() {
        let (x1, y1) = positions[&node.symbol];
        for callee in &node.callees {
            if let Some(&(x2, y2)) = positions.get(callee) {
                svg.push_str(&format!(
                    "<line x1='{x1}' y1='{y1}' x2='{x2}' y2='{y2}' stroke='#888' stroke-width='2' marker-end='url(#arrow)'/>\n",
                    x1=x1, y1=y1, x2=x2, y2=y2
                ));
            }
        }
    }

    // Draw nodes
    for node in call_graph.values() {
        let (x, y) = positions[&node.symbol];
        let body = node.body.as_ref().map(|b| html_escape::encode_safe(b)).unwrap_or_default();
        svg.push_str(&format!(
            "<g>\
                <circle cx='{x}' cy='{y}' r='{r}' fill='#4a90e2' stroke='#222' stroke-width='2'/>\
                <text x='{x}' y='{y}' text-anchor='middle' alignment-baseline='middle' fill='#fff' font-size='14'>{label}</text>\
                <title>{body}</title>\
            </g>\n",
            x=x, y=y, r=node_radius, label=html_escape::encode_safe(&node.display_name), body=body
        ));
    }

    svg.push_str("</svg>\n");
    fs::write(output_path, svg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::fs;
    use tempfile::NamedTempFile;

    #[test]
    fn test_generate_call_graph_dot_tooltip_svg() {
        let mut call_graph = HashMap::new();
        call_graph.insert("f1".to_string(), FunctionNode {
            symbol: "f1".to_string(),
            display_name: "foo".to_string(),
            file_path: "/tmp/foo.rs".to_string(),
            callers: HashSet::new(),
            callees: HashSet::new(),
            range: vec![],
            body: Some("<svg><rect/></svg>".to_string()),
        });
        let tmp = NamedTempFile::new().unwrap();
        generate_call_graph_dot(&call_graph, tmp.path().to_str().unwrap()).unwrap();
        let dot = fs::read_to_string(tmp.path()).unwrap();
        assert!(dot.contains("tooltip=\"<svg><rect/></svg>\""));
    }

    #[test]
    fn test_generate_call_graph_dot_tooltip_invalid_svg() {
        let mut call_graph = HashMap::new();
        call_graph.insert("f2".to_string(), FunctionNode {
            symbol: "f2".to_string(),
            display_name: "bar".to_string(),
            file_path: "/tmp/bar.rs".to_string(),
            callers: HashSet::new(),
            callees: HashSet::new(),
            range: vec![],
            body: Some("not svg".to_string()),
        });
        let tmp = NamedTempFile::new().unwrap();
        generate_call_graph_dot(&call_graph, tmp.path().to_str().unwrap()).unwrap();
        let dot = fs::read_to_string(tmp.path()).unwrap();
        assert!(dot.contains("tooltip=\"[Error: Invalid SVG in body]\""));
    }
}
