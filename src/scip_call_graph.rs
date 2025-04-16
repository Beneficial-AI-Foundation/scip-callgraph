use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use serde::{Deserialize, Serialize};

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
    let mut function_symbols: HashSet<String> = HashSet::new();
    
    // First pass: identify all function symbols and their containing files
    for doc in &scip_data.documents {
        for symbol in &doc.symbols {
            // Check if this is a function-like symbol (kind 12, 17, 80 etc.)
            if is_function_like(symbol.kind) {
                function_symbols.insert(symbol.symbol.clone());
                symbol_to_file.insert(symbol.symbol.clone(), doc.relative_path.clone());
                
                // Initialize node in the call graph
                call_graph.insert(symbol.symbol.clone(), FunctionNode {
                    symbol: symbol.symbol.clone(),
                    display_name: symbol.display_name.clone().unwrap_or_else(|| "unknown".to_string()),
                    file_path: doc.relative_path.clone(),
                    callers: HashSet::new(),
                    callees: HashSet::new(),
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
    
    call_graph
}

/// Check if a symbol kind represents a function-like entity
fn is_function_like(kind: i32) -> bool {
    match kind {
        6 | 12 | 17 | 80 => true,  // Method, Function, etc.
        _ => false,
    }
}

/// Generate DOT format for visualization with Graphviz
pub fn generate_call_graph_dot(call_graph: &HashMap<String, FunctionNode>) -> String {
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
        
        dot.push_str(&format!("  \"{}\" [label=\"{}\\n({})\"];\n", 
            symbol, clean_name, node.file_path));
    }
    
    // Add edges
    for (symbol, node) in call_graph {
        for callee in &node.callees {
            dot.push_str(&format!("  \"{}\" -> \"{}\";\n", symbol, callee));
        }
    }
    
    dot.push_str("}\n");
    dot
}

/// Generate a filtered call graph starting from specific entry points
pub fn generate_filtered_call_graph(
    call_graph: &HashMap<String, FunctionNode>,
    entry_points: &[String],
    max_depth: Option<usize>
) -> HashMap<String, FunctionNode> {
    let mut filtered_graph: HashMap<String, FunctionNode> = HashMap::new();
    let mut visited: HashSet<String> = HashSet::new();
    
    for entry in entry_points {
        if let Some(node) = call_graph.get(entry) {
            traverse_graph(call_graph, node, &mut filtered_graph, &mut visited, 0, max_depth);
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
    max_depth: Option<usize>
) {
    // Check if we've reached max depth or already visited this node
    if max_depth.map_or(false, |max| depth >= max) || visited.contains(&current_node.symbol) {
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
            traverse_graph(full_graph, callee_node, filtered_graph, visited, depth + 1, max_depth);
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
    
    for (_, node) in call_graph {
        if node.callers.is_empty() && !node.callees.is_empty() {
            entry_points += 1;
        } else if !node.callers.is_empty() && node.callees.is_empty() {
            leaf_functions += 1;
        } else if !node.callers.is_empty() && !node.callees.is_empty() {
            internal_functions += 1;
        }
    }
    
    println!("Entry points (functions not called by others): {}", entry_points);
    println!("Leaf functions (functions that don't call others): {}", leaf_functions);
    println!("Internal functions: {}", internal_functions);
    
    // Find the most called functions
    let mut functions_by_caller_count: Vec<_> = call_graph.values().collect();
    functions_by_caller_count.sort_by(|a, b| b.callers.len().cmp(&a.callers.len()));
    
    println!("\nMost called functions:");
    for node in functions_by_caller_count.iter().take(5) {
        if !node.callers.is_empty() {
            println!("  {} (called by {} functions)", node.display_name, node.callers.len());
        }
    }
    
    // Find functions that call the most other functions
    let mut functions_by_callee_count: Vec<_> = call_graph.values().collect();
    functions_by_callee_count.sort_by(|a, b| b.callees.len().cmp(&a.callees.len()));
    
    println!("\nFunctions calling the most other functions:");
    for node in functions_by_callee_count.iter().take(5) {
        if !node.callees.is_empty() {
            println!("  {} (calls {} functions)", node.display_name, node.callees.len());
        }
    }
}

use std::fs::File;
use std::io::Write;

use serde_json::Value;

use crate::scip_reader::{ScipSymbol, SymbolKind};

/// Function to create a dot file for visualizing the call graph
pub fn generate_call_graph(scip_json_file: &str, output_file: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("Generating call graph from {}", scip_json_file);
    
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
                    let symbol_str = occurrence.get("symbol").and_then(|s| s.as_str()).unwrap_or("");
                    let symbol_role = occurrence.get("symbol_roles").and_then(|r| r.as_u64()).unwrap_or(0);
                    
                    // Determine if it's a definition or a reference
                    let is_def = (symbol_role & 1) != 0; // 1 is the bit for definition
                    let is_ref = (symbol_role & 2) != 0; // 2 is the bit for reference
                    
                    // Extract symbol name for display
                    let display_name = if let Some(name) = extract_display_name(symbol_str) {
                        name
                    } else {
                        symbol_str.split('#').last().unwrap_or(symbol_str).to_string()
                    };
                    
                    // Add symbol to the map if it's a definition
                    if is_def {
                        let kind = determine_symbol_kind(symbol_str);
                        symbols.insert(symbol_str.to_string(), ScipSymbol {
                            symbol: symbol_str.to_string(),
                            kind,
                            display_name: Some(display_name),
                        });
                    }
                    
                    // If it's a reference, add a relationship
                    if is_ref {
                        if let Some(container) = occurrence.get("symbol_container").and_then(|s| s.as_str()) {
                            relationships.push((container.to_string(), symbol_str.to_string()));
                        }
                    }
                }
            }
        }
    }
    
    // Generate dot file
    generate_dot_file(output_file, &symbols, &relationships)?;
    
    println!("Call graph generated and saved to {}", output_file);
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
    relationships: &[(String, String)]
) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = File::create(output_file)?;
    
    // Write dot file header
    writeln!(file, "digraph CallGraph {{")?;
    writeln!(file, "  node [shape=box, style=filled, fillcolor=lightblue];")?;
    
    // Process nodes (symbols)
    for (_, symbol) in symbols {
        let label = format!("{}: {}", symbol_kind_to_string(symbol.kind), symbol.display_name.as_deref().unwrap_or("unknown"));
        let node_id = get_node_id(&symbol.symbol);
        writeln!(file, "  {} [label=\"{}\"];", node_id, label)?;
    }
    
    // Process edges (relationships)
    let mut added_edges = HashSet::new();
    for (from, to) in relationships {
        let from_id = get_node_id(from);
        let to_id = get_node_id(to);
        
        let edge_key = format!("{}->{}", from_id, to_id);
        if !added_edges.contains(&edge_key) {
            writeln!(file, "  {} -> {};", from_id, to_id)?;
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
    if clean_symbol.chars().next().map_or(true, |c| !c.is_alphabetic()) {
        format!("n_{}", clean_symbol)
    } else {
        clean_symbol
    }
}

/// Convert a symbol kind to a display string
fn symbol_kind_to_string(kind: SymbolKind) -> &'static str {
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