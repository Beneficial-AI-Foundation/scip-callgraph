use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

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
    pub callers: HashSet<String>, // Symbols that call this function
    pub callees: HashSet<String>, // Symbols that this function calls
    pub range: Vec<i32>,          // Range of the function in the source file
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

    // First pass: identify all function symbols and their containing files
    for doc in &scip_data.documents {
        for symbol in &doc.symbols {
            // Check if this is a function-like symbol (kind 12, 17, 80 etc.)
            if is_function_like(symbol.kind) {
                function_symbols.insert(symbol.symbol.clone());
                symbol_to_file.insert(symbol.symbol.clone(), doc.relative_path.clone());
                symbol_to_kind.insert(symbol.symbol.clone(), symbol.kind);

                // Initialize node in the call graph
                call_graph.insert(
                    symbol.symbol.clone(),
                    FunctionNode {
                        symbol: symbol.symbol.clone(),
                        display_name: symbol
                            .display_name
                            .clone()
                            .unwrap_or_else(|| "unknown".to_string()),
                        file_path: doc.relative_path.clone(),
                        callers: HashSet::new(),
                        callees: HashSet::new(),
                        range: Vec::new(), // Will be filled in the second pass
                    },
                );
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
                    if caller != &occurrence.symbol {
                        // Avoid self-calls for recursion
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
    call_graph
}

/// Check if a symbol kind represents a function-like entity
fn is_function_like(kind: i32) -> bool {
    match kind {
        6 | 12 | 17 | 80 => true, // Method, Function, etc.
        _ => false,
    }
}

/// Helper to convert kind to string
fn kind_to_str(kind: i32) -> &'static str {
    match kind {
        6 => "Method",
        12 => "Constructor",
        17 => "Function",
        80 => "Method",
        49 => "Class",
        _ => "Unknown",
    }
}

/// Generate DOT format for visualization with Graphviz, including kind in the label
pub fn generate_call_graph_dot(call_graph: &HashMap<String, FunctionNode>) -> String {
    // Build a symbol -> kind map for quick lookup
    let mut symbol_to_kind: HashMap<&String, i32> = HashMap::new();
    for node in call_graph.values() {
        // Try to infer kind from display_name if possible, fallback to Unknown
        // But since we don't store kind in FunctionNode, we can't get it directly.
        // So, for now, just use "Unknown" unless you extend FunctionNode to store kind.
        symbol_to_kind.insert(&node.symbol, 17);
    }
    println!("Symbol to kind map: {symbol_to_kind:?}");
    let mut dot = String::new();
    dot.push_str("digraph CallGraph {\n");
    dot.push_str("  node [shape=box, style=filled, fillcolor=lightblue];\n");

    // Add nodes
    for (symbol, node) in call_graph {
        // Create a cleaner display name by removing package info
        let clean_name = if let Some(idx) = node.display_name.rfind('/') {
            &node.display_name[idx + 1..]
        } else {
            &node.display_name
        };

        // Try to get kind from the symbol_to_kind map, fallback to "Unknown"
        let kind_str = symbol_to_kind
            .get(symbol)
            .map(|k| kind_to_str(*k))
            .unwrap_or("Unknown");

        dot.push_str(&format!(
            "  \"{}\" [label=\"{}\\n({})\\n[{}]\\n{:?}\"];\n",
            symbol, clean_name, node.file_path, kind_str, node.range
        ));
    }

    // Add edges
    for (symbol, node) in call_graph {
        for callee in &node.callees {
            dot.push_str(&format!("  \"{symbol}\" -> \"{callee}\";\n"));
        }
    }

    dot.push_str("}\n");
    dot
}

/// Generate a filtered call graph starting from specific entry points
pub fn generate_filtered_call_graph(
    call_graph: &HashMap<String, FunctionNode>,
    entry_points: &[String],
    max_depth: Option<usize>,
) -> HashMap<String, FunctionNode> {
    let mut filtered_graph: HashMap<String, FunctionNode> = HashMap::new();
    let mut visited: HashSet<String> = HashSet::new();

    for entry in entry_points {
        if let Some(node) = call_graph.get(entry) {
            traverse_graph(
                call_graph,
                node,
                &mut filtered_graph,
                &mut visited,
                0,
                max_depth,
            );
        }
    }

    filtered_graph
}

/// Recursively traverse the call graph to build a filtered view
fn traverse_graph(
    full_graph: &HashMap<String, FunctionNode>,
    current_node: &FunctionNode,
    filtered_graph: &mut HashMap<String, FunctionNode>,
    visited: &mut HashSet<String>,
    depth: usize,
    max_depth: Option<usize>,
) {
    // Check if we've reached max depth or already visited this node
    if max_depth.is_some_and(|max| depth >= max) || visited.contains(&current_node.symbol) {
        return;
    }

    // Mark as visited
    visited.insert(current_node.symbol.clone());

    // Add to filtered graph
    if !filtered_graph.contains_key(&current_node.symbol) {
        filtered_graph.insert(current_node.symbol.clone(), current_node.clone());
    }

    // Visit callees
    for callee_symbol in &current_node.callees {
        if let Some(callee_node) = full_graph.get(callee_symbol) {
            // Ensure the relationship is reflected in the filtered graph
            if let Some(filtered_current) = filtered_graph.get_mut(&current_node.symbol) {
                filtered_current.callees.insert(callee_symbol.clone());
            }
            if !filtered_graph.contains_key(callee_symbol) {
                filtered_graph.insert(callee_symbol.clone(), callee_node.clone());
            }
            if let Some(filtered_callee) = filtered_graph.get_mut(callee_symbol) {
                filtered_callee.callers.insert(current_node.symbol.clone());
            }

            // Continue traversal
            traverse_graph(
                full_graph,
                callee_node,
                filtered_graph,
                visited,
                depth + 1,
                max_depth,
            );
        }
    }
}

/// Generate a human-readable call graph summary
pub fn print_call_graph_summary(call_graph: &HashMap<String, FunctionNode>) {
    println!("Call Graph Summary");
    println!("=================");
    println!("Total functions: {}", call_graph.len());

    let mut entry_points = 0;
    let mut leaf_functions = 0;
    let mut internal_functions = 0;

    for node in call_graph.values() {
        if node.callers.is_empty() && !node.callees.is_empty() {
            entry_points += 1;
        } else if !node.callers.is_empty() && node.callees.is_empty() {
            leaf_functions += 1;
        } else if !node.callers.is_empty() && !node.callees.is_empty() {
            internal_functions += 1;
        }
    }

    println!(
        "Entry points (functions not called by others): {entry_points}"
    );
    println!(
        "Leaf functions (functions that don't call others): {leaf_functions}"
    );
    println!("Internal functions: {internal_functions}");

    // Find the most called functions
    let mut functions_by_caller_count: Vec<_> = call_graph.values().collect();
    functions_by_caller_count.sort_by(|a, b| b.callers.len().cmp(&a.callers.len()));

    println!("\nMost called functions:");
    for node in functions_by_caller_count.iter().take(5) {
        if !node.callers.is_empty() {
            println!(
                "  {} (called by {} functions)",
                node.display_name,
                node.callers.len()
            );
        }
    }

    // Find functions that call the most other functions
    let mut functions_by_callee_count: Vec<_> = call_graph.values().collect();
    functions_by_callee_count.sort_by(|a, b| b.callees.len().cmp(&a.callees.len()));

    println!("\nFunctions calling the most other functions:");
    for node in functions_by_callee_count.iter().take(5) {
        if !node.callees.is_empty() {
            println!(
                "  {} (calls {} functions)",
                node.display_name,
                node.callees.len()
            );
        }
    }
}

use std::fs::File;
use std::io::Write;

use serde_json::Value;

use crate::scip_reader::{ScipSymbol, SymbolKind};

/// Function to create a dot file for visualizing the call graph
pub fn generate_call_graph(
    scip_json_file: &str,
    output_file: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Generating call graph from {scip_json_file}");

    // Read the SCIP JSON file
    let file = File::open(scip_json_file)?;
    let json: Value = serde_json::from_reader(file)?;

    // Extract symbols and relationships
    let mut symbols = HashMap::new();
    let mut relationships = Vec::new();

    // Process documents
    if let Some(documents) = json.get("documents").and_then(|d| d.as_array()) {
        for doc in documents {
            // Process symbols
            if let Some(occurrences) = doc.get("occurrences").and_then(|o| o.as_array()) {
                for occurrence in occurrences {
                    let symbol_str = occurrence
                        .get("symbol")
                        .and_then(|s| s.as_str())
                        .unwrap_or("");
                    let symbol_role = occurrence
                        .get("symbol_roles")
                        .and_then(|r| r.as_u64())
                        .unwrap_or(0);

                    // Determine if it's a definition or a reference
                    let is_def = (symbol_role & 1) != 0; // 1 is the bit for definition
                    let is_ref = (symbol_role & 2) != 0; // 2 is the bit for reference

                    // Extract symbol name for display
                    let display_name = if let Some(name) = extract_display_name(symbol_str) {
                        name
                    } else {
                        symbol_str
                            .split('#')
                            .next_back()
                            .unwrap_or(symbol_str)
                            .to_string()
                    };

                    // Add symbol to the map if it's a definition
                    if is_def {
                        let kind = determine_symbol_kind(symbol_str);
                        symbols.insert(
                            symbol_str.to_string(),
                            ScipSymbol {
                                symbol: symbol_str.to_string(),
                                kind,
                                display_name: Some(display_name),
                            },
                        );
                    }

                    // If it's a reference, add a relationship
                    if is_ref {
                        if let Some(container) =
                            occurrence.get("symbol_container").and_then(|s| s.as_str())
                        {
                            relationships.push((container.to_string(), symbol_str.to_string()));
                        }
                    }
                }
            }
        }
    }

    // Generate dot file
    generate_dot_file(output_file, &symbols, &relationships)?;

    println!("Call graph generated and saved to {output_file}");
    Ok(())
}

/// Extract a display name from a SCIP symbol
fn extract_display_name(symbol: &str) -> Option<String> {
    // Extract a more readable name from the symbol
    let parts: Vec<&str> = symbol.split('#').collect();
    if parts.len() < 2 {
        return None;
    }

    let last_part = parts[1];
    if last_part.contains('(') {
        // It's likely a method or function
        Some(last_part.split('(').next().unwrap_or(last_part).to_string())
    } else if last_part.is_empty() {
        // It's likely a class or module
        let module_parts: Vec<&str> = parts[0].split('/').collect();
        Some(module_parts.last().unwrap_or(&parts[0]).to_string())
    } else {
        Some(last_part.to_string())
    }
}

/// Determine the kind of symbol based on its string representation
fn determine_symbol_kind(symbol: &str) -> SymbolKind {
    if symbol.contains("()") {
        if symbol.contains('#') {
            SymbolKind::Method
        } else {
            SymbolKind::Function
        }
    } else if symbol.ends_with("#") {
        SymbolKind::Class
    } else {
        SymbolKind::Unknown
    }
}

/// Generate a GraphViz dot file for the call graph
fn generate_dot_file(
    output_file: &str,
    symbols: &HashMap<String, ScipSymbol>,
    relationships: &[(String, String)],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = File::create(output_file)?;

    // Write dot file header
    writeln!(file, "digraph CallGraph {{")?;
    writeln!(
        file,
        "  node [shape=box, style=filled, fillcolor=lightblue];"
    )?;

    // Process nodes (symbols)
    for symbol in symbols.values() {
        println!("Processing symbol: {:?}", symbol.display_name);
        let label = format!(
            "{}: {}",
            symbol_kind_to_string(symbol.kind),
            symbol.display_name.as_deref().unwrap_or("unknown")
        );
        let node_id = get_node_id(&symbol.symbol);
        writeln!(file, "  {node_id} [label=\"{label}\"];")?;
    }

    // Process edges (relationships)
    let mut added_edges = HashSet::new();
    for (from, to) in relationships {
        let from_id = get_node_id(from);
        let to_id = get_node_id(to);

        let edge_key = format!("{from_id}->{to_id}");
        if !added_edges.contains(&edge_key) {
            writeln!(file, "  {from_id} -> {to_id};")?;
            added_edges.insert(edge_key);
        }
    }

    // Close the graph
    writeln!(file, "}}")?;

    Ok(())
}

/// Convert a symbol to a valid dot node identifier
fn get_node_id(symbol: &str) -> String {
    // Replace invalid characters with underscore
    let clean_symbol = symbol
        .replace(|c: char| !c.is_alphanumeric() && c != '_', "_")
        .replace("__", "_");

    // Ensure it starts with a letter
    if clean_symbol
        .chars()
        .next()
        .is_none_or(|c| !c.is_alphabetic())
    {
        format!("n_{clean_symbol}")
    } else {
        clean_symbol
    }
}

/// Convert a symbol kind to a display string
fn symbol_kind_to_string(kind: SymbolKind) -> &'static str {
    println!("Symbol kind: {kind:?}");
    match kind {
        SymbolKind::Function => "Function",
        SymbolKind::Method => "Method",
        SymbolKind::Class => "Class",
        SymbolKind::Interface => "Interface",
        SymbolKind::Enum => "Enum",
        SymbolKind::TypeParameter => "TypeParam",
        SymbolKind::Parameter => "Param",
        SymbolKind::Variable => "Var",
        SymbolKind::Field => "Field",
        SymbolKind::Unknown => "Unknown",
    }
}

use serde::ser::{SerializeStruct, Serializer};

#[derive(Debug)]
pub struct Atom {
    pub identifier: String,
    pub statement_type: String,
    pub deps: Vec<String>,
    pub body: String,
}

impl Serialize for Atom {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut s = serializer.serialize_struct("Atom", 4)?;
        s.serialize_field("identifier", &self.identifier)?;
        s.serialize_field("statement_type", &self.statement_type)?;
        s.serialize_field("deps", &self.deps)?;
        s.serialize_field("body", &self.body)?;
        s.end()
    }
}

fn parse_dot_file(
    dot_file: &str,
) -> Result<(HashMap<String, String>, Vec<(String, String)>), Box<dyn std::error::Error>> {
    use std::fs::File;
    use std::io::{BufRead, BufReader};
    let file = File::open(dot_file)?;
    let reader = BufReader::new(file);
    let mut nodes: HashMap<String, String> = HashMap::new(); // id -> label
    let mut edges: Vec<(String, String)> = Vec::new(); // (from, to)
    for line in reader.lines() {
        let line = line?;
        let line = line.trim();
        if line.starts_with("//") || line.is_empty() {
            continue;
        }
        if let Some(idx) = line.find('[') {
            let id = line[..idx].trim().trim_end_matches(';');
            if let Some(label_start) = line.find("label=\"") {
                let label_end = line[label_start + 7..].find('"').unwrap_or(0) + label_start + 7;
                let label = &line[label_start + 7..label_end];
                nodes.insert(id.to_string(), label.to_string());
            }
        } else if line.contains("->") {
            let parts: Vec<&str> = line.split("->").collect();
            if parts.len() == 2 {
                let from = parts[0].trim().trim_end_matches(';');
                let to = parts[1].trim().trim_end_matches(';');
                edges.push((from.to_string(), to.to_string()));
            }
        }
    }
    Ok((nodes, edges))
}

fn parse_scip_symbol_ranges(
    scip_json: &str,
) -> Result<HashMap<String, (String, Vec<i32>)>, Box<dyn std::error::Error>> {
    use serde_json;
    use std::fs::File;
    let scip: serde_json::Value = serde_json::from_reader(File::open(scip_json)?)?;
    let mut symbol_to_range: HashMap<String, (String, Vec<i32>)> = HashMap::new();
    if let Some(docs) = scip
        .as_object()
        .and_then(|o| o.get("documents"))
        .and_then(|d| d.as_array())
    {
        for doc in docs {
            let rel_path = doc
                .get("relative_path")
                .and_then(|p| p.as_str())
                .unwrap_or("");
            if let Some(symbols) = doc.get("symbols").and_then(|s| s.as_array()) {
                for sym in symbols {
                    if let Some(symbol) = sym.get("symbol").and_then(|s| s.as_str()) {
                        if let Some(occurrences) = doc.get("occurrences").and_then(|o| o.as_array())
                        {
                            for occ in occurrences {
                                if occ.get("symbol").and_then(|s| s.as_str()) == Some(symbol)
                                    && occ.get("symbol_roles").and_then(|r| r.as_i64()) == Some(1) {
                                        if let Some(range) =
                                            occ.get("range").and_then(|r| r.as_array())
                                        {
                                            let range_vec = range
                                                .iter()
                                                .filter_map(|v| v.as_i64().map(|x| x as i32))
                                                .collect::<Vec<_>>();
                                            symbol_to_range.insert(
                                                symbol.to_string(),
                                                (rel_path.to_string(), range_vec),
                                            );
                                            break;
                                        }
                                    }
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(symbol_to_range)
}

fn parse_scip_symbol_kinds(
    scip_json: &str,
) -> Result<HashMap<String, i32>, Box<dyn std::error::Error>> {
    use serde_json;
    use std::fs::File;
    let scip: serde_json::Value = serde_json::from_reader(File::open(scip_json)?)?;
    let mut symbol_to_kind: HashMap<String, i32> = HashMap::new();
    if let Some(docs) = scip
        .as_object()
        .and_then(|o| o.get("documents"))
        .and_then(|d| d.as_array())
    {
        for doc in docs {
            if let Some(symbols) = doc.get("symbols").and_then(|s| s.as_array()) {
                for sym in symbols {
                    if let Some(symbol) = sym.get("symbol").and_then(|s| s.as_str()) {
                        if let Some(kind) = sym.get("kind").and_then(|k| k.as_i64()) {
                            symbol_to_kind.insert(symbol.to_string(), kind as i32);
                        }
                    }
                }
            }
        }
    }
    Ok(symbol_to_kind)
}

fn kind_to_statement_type(kind: i32) -> &'static str {
    match kind {
        17 => "function", // Function
        80 => "method",   // Method
        49 => "struct",   // Class
        _ => "unknown",
    }
}

fn extract_body_from_file(src_path: &std::path::Path, range: &[i32]) -> String {
    if let Ok(src) = std::fs::read_to_string(src_path) {
        if range.len() == 4 {
            let start_line = range[0] as usize;
            let start_col = range[1] as usize;
            let end_line = range[2] as usize;
            let end_col = range[3] as usize;
            let lines: Vec<&str> = src.lines().collect();
            if start_line < lines.len() && end_line < lines.len() {
                let mut extracted = String::new();
                for (i, line) in lines.iter().enumerate().take(end_line + 1).skip(start_line) {
                    if i == start_line && i == end_line {
                        extracted.push_str(&line[start_col..end_col]);
                    } else if i == start_line {
                        extracted.push_str(&line[start_col..]);
                        extracted.push('\n');
                    } else if i == end_line {
                        extracted.push_str(&line[..end_col]);
                    } else {
                        extracted.push_str(line);
                        extracted.push('\n');
                    }
                }
                return extracted;
            }
        }
    }
    String::new()
}

pub fn dot_to_atoms_json_with_body(
    dot_file: &str,
    json_file: &str,
    scip_json: &str,
    src_root: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use serde_json;
    use std::collections::HashMap;
    use std::fs::File;
    use std::path::Path;
    // Parse DOT and SCIP
    let (nodes, edges) = parse_dot_file(dot_file)?;
    let symbol_to_range = parse_scip_symbol_ranges(scip_json)?;
    let symbol_to_kind = parse_scip_symbol_kinds(scip_json)?;
    // Build dependency map
    let mut deps_map: HashMap<String, Vec<String>> = HashMap::new();
    for (from, to) in &edges {
        deps_map.entry(to.clone()).or_default().push(from.clone());
        deps_map.entry(from.clone()).or_default();
    }
    for node in nodes.keys() {
        deps_map.entry(node.clone()).or_default();
    }
    // Build atoms
    let atoms: Vec<Atom> = nodes.keys().map(|id| {
            // Find the best matching symbol for this node id
            let mut statement_type = "unknown";
            let mut body = String::new();
            for (symbol, (rel_path, range)) in &symbol_to_range {
                if symbol
                    .replace(|c: char| !c.is_alphanumeric(), "_")
                    .contains(id)
                    || id.contains(&symbol.replace(|c: char| !c.is_alphanumeric(), "_"))
                {
                    // Get kind for this symbol
                    if let Some(kind) = symbol_to_kind.get(symbol) {
                        statement_type = kind_to_statement_type(*kind);
                    }
                    // Extract body if function or method
                    if statement_type == "function" || statement_type == "method" {
                        let src_path = Path::new(src_root).join(rel_path);
                        body = extract_body_from_file(&src_path, range);
                    }
                    break;
                }
            }
            Atom {
                identifier: id.clone(),
                statement_type: statement_type.to_string(),
                deps: deps_map.get(id).cloned().unwrap_or_default(),
                body,
            }
        })
        .collect();
    // Write to JSON
    let out = File::create(json_file)?;
    serde_json::to_writer_pretty(out, &atoms)?;
    Ok(())
}

/// Parse a DOT file and generate a JSON file containing Atoms
pub fn dot_to_atoms_json(
    dot_file: &str,
    json_file: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use serde_json;
    use std::collections::HashMap;
    use std::fs::File;
    use std::io::{BufRead, BufReader};

    let file = File::open(dot_file)?;
    let reader = BufReader::new(file);

    let mut nodes: HashMap<String, String> = HashMap::new(); // id -> label
    let mut edges: Vec<(String, String)> = Vec::new(); // (from, to)

    for line in reader.lines() {
        let line = line?;
        let line = line.trim();
        if line.starts_with("//") || line.is_empty() {
            continue;
        }
        // Node:   id [label="..."];
        if let Some(idx) = line.find('[') {
            let id = line[..idx].trim().trim_end_matches(';');
            if let Some(label_start) = line.find("label=\"") {
                let label_end = line[label_start + 7..].find('"').unwrap_or(0) + label_start + 7;
                let label = &line[label_start + 7..label_end];
                nodes.insert(id.to_string(), label.to_string());
            }
        } else if line.contains("->") {
            // Edge:   from -> to;
            let parts: Vec<&str> = line.split("->").collect();
            if parts.len() == 2 {
                let from = parts[0].trim().trim_end_matches(';');
                let to = parts[1].trim().trim_end_matches(';');
                edges.push((from.to_string(), to.to_string()));
            }
        }
    }

    // Build dependency map: for each node, collect incoming edges (deps)
    let mut deps_map: HashMap<String, Vec<String>> = HashMap::new();
    for (from, to) in &edges {
        deps_map.entry(to.clone()).or_default().push(from.clone());
        deps_map.entry(from.clone()).or_default();
    }
    for node in nodes.keys() {
        deps_map.entry(node.clone()).or_default();
    }

    // Build atoms
    let atoms: Vec<Atom> = nodes
        .iter()
        .map(|(id, label)| {
            let statement_type = if label.contains(": Function") {
                "function"
            } else if label.contains(": Method") {
                "method"
            } else if label.contains(": Class") {
                "class"
            } else {
                "unknown"
            };
            Atom {
                identifier: id.clone(),
                statement_type: statement_type.to_string(),
                deps: deps_map.get(id).cloned().unwrap_or_default(),
                body: String::new(),
            }
        })
        .collect();

    // Write to JSON
    let out = File::create(json_file)?;
    serde_json::to_writer_pretty(out, &atoms)?;
    Ok(())
}
