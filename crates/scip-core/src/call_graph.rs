//! Core call graph building and analysis.
//!
//! This module provides the core functionality for building call graphs from SCIP data:
//! - `build_call_graph` - Build a call graph from SCIP index
//! - `detect_function_mode` - Detect Verus function mode (exec/proof/spec)
//! - `parse_function_sections` - Parse requires/ensures/body sections
//! - `classify_call_location` - Classify where calls occur (precondition/postcondition/inner)
//! - `generate_filtered_call_graph` - Create depth-limited subgraphs
//! - `print_call_graph_summary` - Print human-readable summary

use crate::parser::{extract_display_name_from_symbol, extract_path_info_from_symbol};
use crate::types::{
    CallLocation, CalleeOccurrence, FunctionMode, FunctionNode, FunctionSections, ScipIndex,
};
use log::{debug, info};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

/// Compiled regex for removing generic type parameters from paths
fn generics_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"<[^>]*>").unwrap())
}

/// Check if a symbol kind represents a function-like entity.
///
/// SCIP kind values:
/// - 6: Constructor
/// - 12: Function
/// - 17: Macro
/// - 80: Method
pub fn is_function_like(kind: i32) -> bool {
    matches!(kind, 6 | 12 | 17 | 80)
}

/// Detect the Verus function mode from the function signature/body.
///
/// Verus functions can be:
/// - `fn` or `exec fn` - executable code
/// - `proof fn` - proof functions (lemmas)
/// - `spec fn` or `open spec fn` or `closed spec fn` - specification functions
pub fn detect_function_mode(body: &str) -> FunctionMode {
    let signature_area: String = body.lines().take(5).collect::<Vec<_>>().join(" ");
    let signature_lower = signature_area.to_lowercase();

    if signature_lower.contains("spec fn")
        || signature_lower.contains("spec(checked) fn")
        || signature_lower.contains("open spec fn")
        || signature_lower.contains("closed spec fn")
    {
        return FunctionMode::Spec;
    }

    if signature_lower.contains("proof fn") {
        return FunctionMode::Proof;
    }

    FunctionMode::Exec
}

/// Parse a function body to find the line ranges for requires, ensures, and body sections.
///
/// # Arguments
/// * `body` - The full function body text (including signature)
/// * `func_start_line` - The 0-based line number where the function starts
///
/// # Returns
/// A `FunctionSections` struct with the identified line ranges
pub fn parse_function_sections(body: &str, func_start_line: i32) -> FunctionSections {
    let mut sections = FunctionSections {
        start_line: func_start_line,
        ..Default::default()
    };

    let lines: Vec<&str> = body.lines().collect();

    let mut in_requires = false;
    let mut in_ensures = false;
    let mut requires_start: Option<i32> = None;
    let mut ensures_start: Option<i32> = None;
    let mut brace_depth: i32 = 0;
    let mut found_body_start = false;

    for (i, line) in lines.iter().enumerate() {
        let line_num = func_start_line + i as i32;
        let trimmed = line.trim();

        // Check for section keywords
        if trimmed.starts_with("requires") && !found_body_start {
            if in_ensures {
                if let Some(start) = ensures_start {
                    sections.ensures_range = Some((start, line_num - 1));
                }
                in_ensures = false;
            }
            in_requires = true;
            requires_start = Some(line_num);
        } else if (trimmed.starts_with("ensures") || trimmed.starts_with("decreases"))
            && !found_body_start
        {
            if in_requires {
                if let Some(start) = requires_start {
                    sections.requires_range = Some((start, line_num - 1));
                }
                in_requires = false;
            }
            if in_ensures {
                if let Some(start) = ensures_start {
                    sections.ensures_range = Some((start, line_num - 1));
                }
            }
            in_ensures = true;
            ensures_start = Some(line_num);
        }

        // Track brace depth to find body start
        for ch in line.chars() {
            if ch == '{' {
                if brace_depth == 0 && !found_body_start {
                    found_body_start = true;
                    sections.body_start_line = Some(line_num);

                    if in_requires {
                        if let Some(start) = requires_start {
                            sections.requires_range = Some((start, line_num - 1));
                        }
                        in_requires = false;
                    }
                    if in_ensures {
                        if let Some(start) = ensures_start {
                            sections.ensures_range = Some((start, line_num - 1));
                        }
                        in_ensures = false;
                    }
                }
                brace_depth += 1;
            } else if ch == '}' {
                brace_depth = brace_depth.saturating_sub(1);
            }
        }
    }

    sections
}

/// Classify a call occurrence based on its line number and the function sections.
pub fn classify_call_location(call_line: i32, sections: &FunctionSections) -> CallLocation {
    if let Some((start, end)) = sections.requires_range {
        if call_line >= start && call_line <= end {
            return CallLocation::Precondition;
        }
    }

    if let Some((start, end)) = sections.ensures_range {
        if call_line >= start && call_line <= end {
            return CallLocation::Postcondition;
        }
    }

    CallLocation::Inner
}

/// Build a call graph from SCIP JSON data
pub fn build_call_graph(scip_data: &ScipIndex) -> HashMap<String, FunctionNode> {
    let mut call_graph: HashMap<String, FunctionNode> = HashMap::new();
    let mut symbol_to_file: HashMap<String, String> = HashMap::new();
    let mut symbol_to_kind: HashMap<String, i32> = HashMap::new();
    let mut function_symbols: HashSet<String> = HashSet::new();

    // Pre-pass: Find where each symbol is DEFINED (symbol_roles == 1)
    let mut symbol_to_def_file: HashMap<String, (String, String)> = HashMap::new();
    for doc in &scip_data.documents {
        let project_root = &scip_data.metadata.project_root;
        let rel_path = doc.relative_path.trim_start_matches('/');
        let abs_path = format!("{project_root}/{rel_path}");

        for occurrence in &doc.occurrences {
            let is_definition = occurrence.symbol_roles.unwrap_or(0) & 1 == 1;
            if is_definition {
                symbol_to_def_file.insert(
                    occurrence.symbol.clone(),
                    (abs_path.clone(), rel_path.to_string()),
                );
            }
        }
    }
    debug!(
        "Pre-pass: Found {} symbol definitions",
        symbol_to_def_file.len()
    );

    // First pass: identify all LOCAL function symbols
    for doc in &scip_data.documents {
        for symbol in &doc.symbols {
            if is_function_like(symbol.kind) {
                let (abs_path, rel_path) =
                    if let Some((def_abs, def_rel)) = symbol_to_def_file.get(&symbol.symbol) {
                        (def_abs.clone(), def_rel.clone())
                    } else {
                        continue;
                    };

                function_symbols.insert(symbol.symbol.clone());
                symbol_to_file.insert(symbol.symbol.clone(), abs_path.clone());
                symbol_to_kind.insert(symbol.symbol.clone(), symbol.kind);

                call_graph.insert(
                    symbol.symbol.clone(),
                    FunctionNode {
                        symbol: symbol.symbol.clone(),
                        display_name: symbol
                            .display_name
                            .clone()
                            .unwrap_or_else(|| "unknown".to_string()),
                        file_path: abs_path,
                        relative_path: rel_path,
                        callers: HashSet::new(),
                        callees: HashSet::new(),
                        callee_occurrences: Vec::new(),
                        range: Vec::new(),
                        body: None,
                    },
                );
            }
        }
    }

    debug!(
        "First pass: Found {} local function symbols",
        function_symbols.len()
    );

    // Pass 1.5: Identify external function symbols
    let mut external_function_symbols: HashSet<String> = HashSet::new();
    let mut external_display_names: HashMap<String, String> = HashMap::new();

    for doc in &scip_data.documents {
        for symbol in &doc.symbols {
            if is_function_like(symbol.kind) && !function_symbols.contains(&symbol.symbol) {
                external_function_symbols.insert(symbol.symbol.clone());
                if let Some(name) = &symbol.display_name {
                    external_display_names.insert(symbol.symbol.clone(), name.clone());
                }
            }
        }
    }

    for doc in &scip_data.documents {
        for occurrence in &doc.occurrences {
            let is_definition = occurrence.symbol_roles.unwrap_or(0) & 1 == 1;
            let symbol = &occurrence.symbol;

            if is_definition
                || function_symbols.contains(symbol)
                || external_function_symbols.contains(symbol)
            {
                continue;
            }

            if (symbol.contains("()") || symbol.ends_with("."))
                && (symbol.contains('#') || symbol.contains('/'))
                && !symbol.contains("().(")
            {
                external_function_symbols.insert(symbol.clone());
            }
        }
    }

    debug!(
        "Pass 1.5: Found {} external function symbols",
        external_function_symbols.len()
    );

    // Create placeholder nodes for external functions
    for symbol in &external_function_symbols {
        let display_name = external_display_names
            .get(symbol)
            .cloned()
            .unwrap_or_else(|| extract_display_name_from_symbol(symbol));

        let (relative_path, _file_name, _parent_folder) = extract_path_info_from_symbol(symbol);

        call_graph.insert(
            symbol.clone(),
            FunctionNode {
                symbol: symbol.clone(),
                display_name,
                file_path: format!("external:{}", symbol),
                relative_path,
                callers: HashSet::new(),
                callees: HashSet::new(),
                callee_occurrences: Vec::new(),
                range: Vec::new(),
                body: None,
            },
        );
    }

    let all_function_symbols: HashSet<String> = function_symbols
        .union(&external_function_symbols)
        .cloned()
        .collect();

    // Second pass: analyze occurrences to build the call graph
    for doc in &scip_data.documents {
        let mut current_function: Option<String> = None;

        let mut ordered_occurrences = doc.occurrences.clone();
        ordered_occurrences.sort_by(|a, b| {
            let a_start = (a.range[0], a.range[1]);
            let b_start = (b.range[0], b.range[1]);
            a_start.cmp(&b_start)
        });

        for occurrence in &ordered_occurrences {
            let is_definition = occurrence.symbol_roles.unwrap_or(0) & 1 == 1;

            if is_definition && function_symbols.contains(&occurrence.symbol) {
                current_function = Some(occurrence.symbol.clone());
                if let Some(node) = call_graph.get_mut(&occurrence.symbol) {
                    node.range = occurrence.range.clone();
                }
            }

            if !is_definition && all_function_symbols.contains(&occurrence.symbol) {
                if let Some(caller) = &current_function {
                    if caller != &occurrence.symbol {
                        if let Some(caller_node) = call_graph.get_mut(caller) {
                            caller_node.callees.insert(occurrence.symbol.clone());
                            let call_line = occurrence.range.first().copied().unwrap_or(0);
                            caller_node.callee_occurrences.push(CalleeOccurrence {
                                symbol: occurrence.symbol.clone(),
                                line: call_line,
                                location: None,
                            });
                        }

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
        if !node.range.is_empty() {
            let file_path = &node.file_path;

            let clean_path = if file_path.starts_with("file://") {
                file_path.trim_start_matches("file://")
            } else {
                file_path
            };

            let abs_path = Path::new(clean_path);
            debug!("Trying to read file: {clean_path}");

            if let Ok(contents) = fs::read_to_string(abs_path) {
                let lines: Vec<&str> = contents.lines().collect();

                let display_name = &node.display_name;
                let range = &node.range;
                debug!("Function: {display_name}, Range: {range:?}");

                if !node.range.is_empty() {
                    let start_line = node.range[0] as usize;

                    if start_line < lines.len() {
                        let mut body_lines = Vec::new();
                        let mut open_braces = 0;
                        let mut found_first_brace = false;

                        body_lines.push(lines[start_line]);

                        for (line_idx, line) in lines.iter().enumerate().skip(start_line) {
                            if line_idx == start_line {
                                if line.contains('{') {
                                    found_first_brace = true;
                                    open_braces = line.matches('{').count();
                                    open_braces =
                                        open_braces.saturating_sub(line.matches('}').count());
                                }
                                continue;
                            }

                            if !found_first_brace {
                                if line.contains('{') {
                                    found_first_brace = true;
                                    open_braces = line.matches('{').count();
                                    open_braces =
                                        open_braces.saturating_sub(line.matches('}').count());
                                }
                                body_lines.push(line);
                            } else {
                                open_braces += line.matches('{').count();
                                open_braces = open_braces.saturating_sub(line.matches('}').count());
                                body_lines.push(line);
                                if open_braces == 0 {
                                    break;
                                }
                            }
                        }

                        let full_body = body_lines.join("\n");
                        let body_len = full_body.len();
                        node.body = Some(full_body.clone());
                        let display_name = &node.display_name;
                        debug!("Extracted body for {display_name}, length: {body_len}");

                        let sections = parse_function_sections(&full_body, node.range[0]);
                        for occurrence in &mut node.callee_occurrences {
                            occurrence.location =
                                Some(classify_call_location(occurrence.line, &sections));
                        }

                        debug!(
                            "Classified {} callee occurrences for {display_name}: {:?}",
                            node.callee_occurrences.len(),
                            sections
                        );
                    }
                }
            } else {
                debug!("Failed to read file: {clean_path}");
            }
        }
    }

    // Fourth pass: Default unclassified callee occurrences to Inner
    for node in call_graph.values_mut() {
        for occurrence in &mut node.callee_occurrences {
            if occurrence.location.is_none() {
                occurrence.location = Some(CallLocation::Inner);
            }
        }
    }

    call_graph
}

/// Convert a SCIP symbol to a clean path format with display name
pub fn symbol_to_path(symbol: &str, display_name: &str) -> String {
    let mut parts = symbol.split_whitespace();
    let mut s = symbol;
    if parts.next() == Some("rust-analyzer") && parts.next() == Some("cargo") {
        if let Some(rest) = symbol.find("cargo ").and_then(|pos| symbol.get(pos + 6..)) {
            s = rest;
        }
    }

    if let Some(pos) = s.find(|c: char| c.is_ascii_digit()) {
        if let Some(space_pos) = s[pos..].find(' ') {
            s = s[(pos + space_pos + 1)..].trim();
        }
    }

    let path = s
        .replace(['/', '#'], "::")
        .replace("impl#", "")
        .replace('`', "");

    let path = generics_regex().replace_all(&path, "");
    let path = path.trim_end_matches('.').trim_end_matches("()");

    if path.ends_with(&format!("::{display_name}")) {
        path.to_string()
    } else {
        format!("{path}::{display_name}")
    }
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
    if max_depth.is_some_and(|max| depth >= max) || visited.contains(&current_node.symbol) {
        return;
    }

    visited.insert(current_node.symbol.clone());

    if !filtered_graph.contains_key(&current_node.symbol) {
        filtered_graph.insert(current_node.symbol.clone(), current_node.clone());
    }

    for callee_symbol in &current_node.callees {
        if let Some(callee_node) = full_graph.get(callee_symbol) {
            if let Some(filtered_current) = filtered_graph.get_mut(&current_node.symbol) {
                filtered_current.callees.insert(callee_symbol.clone());
            }
            if !filtered_graph.contains_key(callee_symbol) {
                filtered_graph.insert(callee_symbol.clone(), callee_node.clone());
            }
            if let Some(filtered_callee) = filtered_graph.get_mut(callee_symbol) {
                filtered_callee.callers.insert(current_node.symbol.clone());
            }

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

/// Print a human-readable call graph summary
pub fn print_call_graph_summary(call_graph: &HashMap<String, FunctionNode>) {
    info!("Call Graph Summary");
    info!("=================");
    info!("Total functions: {}", call_graph.len());

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

    info!("Entry points (functions not called by others): {entry_points}");
    info!("Leaf functions (functions that don't call others): {leaf_functions}");
    info!("Internal functions: {internal_functions}");

    let mut functions_by_caller_count: Vec<_> = call_graph.values().collect();
    functions_by_caller_count.sort_by(|a, b| b.callers.len().cmp(&a.callers.len()));

    info!("\nMost called functions:");
    for node in functions_by_caller_count.iter().take(5) {
        if !node.callers.is_empty() {
            info!(
                "  {} (called by {} functions)",
                node.display_name,
                node.callers.len()
            );
        }
    }

    let mut functions_by_callee_count: Vec<_> = call_graph.values().collect();
    functions_by_callee_count.sort_by(|a, b| b.callees.len().cmp(&a.callees.len()));

    info!("\nFunctions calling the most other functions:");
    for node in functions_by_callee_count.iter().take(5) {
        if !node.callees.is_empty() {
            info!(
                "  {} (calls {} functions)",
                node.display_name,
                node.callees.len()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==========================================================================
    // is_function_like tests - SCIP kind identification
    // ==========================================================================

    #[test]
    fn test_is_function_like_method() {
        assert!(is_function_like(6)); // Constructor
        assert!(is_function_like(80)); // Method (Rust-specific)
    }

    #[test]
    fn test_is_function_like_function() {
        assert!(is_function_like(12)); // Function
        assert!(is_function_like(17)); // Macro
    }

    #[test]
    fn test_is_function_like_non_function() {
        assert!(!is_function_like(0)); // Unknown
        assert!(!is_function_like(26)); // TypeParameter
        assert!(!is_function_like(100)); // Some other kind
    }

    // ==========================================================================
    // detect_function_mode tests - Verus mode detection
    // ==========================================================================

    #[test]
    fn test_detect_function_mode_exec_default() {
        let body = "fn my_function() -> i32 { 42 }";
        assert_eq!(detect_function_mode(body), FunctionMode::Exec);
    }

    #[test]
    fn test_detect_function_mode_exec_explicit() {
        let body = "exec fn my_function() -> i32 { 42 }";
        assert_eq!(detect_function_mode(body), FunctionMode::Exec);
    }

    #[test]
    fn test_detect_function_mode_proof() {
        let body = "proof fn lemma_something()
            requires x > 0
        { }";
        assert_eq!(detect_function_mode(body), FunctionMode::Proof);
    }

    #[test]
    fn test_detect_function_mode_spec() {
        let body = "spec fn pure_computation(x: nat) -> nat { x + 1 }";
        assert_eq!(detect_function_mode(body), FunctionMode::Spec);
    }

    #[test]
    fn test_detect_function_mode_open_spec() {
        let body = "pub open spec fn visible_spec() -> bool { true }";
        assert_eq!(detect_function_mode(body), FunctionMode::Spec);
    }

    #[test]
    fn test_detect_function_mode_closed_spec() {
        let body = "closed spec fn hidden_spec() -> nat { 0 }";
        assert_eq!(detect_function_mode(body), FunctionMode::Spec);
    }

    #[test]
    fn test_detect_function_mode_spec_checked() {
        let body = "spec(checked) fn checked_spec() -> bool { true }";
        assert_eq!(detect_function_mode(body), FunctionMode::Spec);
    }

    #[test]
    fn test_detect_function_mode_multiline_signature() {
        // Mode keyword might be on second line
        let body = "pub
            proof fn my_lemma()
            { }";
        assert_eq!(detect_function_mode(body), FunctionMode::Proof);
    }

    // ==========================================================================
    // parse_function_sections tests - Verus clause detection
    // ==========================================================================

    #[test]
    fn test_parse_function_sections_no_specs() {
        let body = "fn simple() {
            println!(\"hello\");
        }";
        let sections = parse_function_sections(body, 0);

        assert!(sections.requires_range.is_none());
        assert!(sections.ensures_range.is_none());
        assert!(sections.body_start_line.is_some());
    }

    #[test]
    fn test_parse_function_sections_with_requires() {
        let body = "fn with_precondition(x: i32)
            requires x > 0
        {
            x + 1
        }";
        let sections = parse_function_sections(body, 10);

        assert!(sections.requires_range.is_some());
        let (start, _end) = sections.requires_range.unwrap();
        assert_eq!(start, 11); // Line after fn signature
    }

    #[test]
    fn test_parse_function_sections_with_ensures() {
        let body = "fn with_postcondition(x: i32) -> i32
            ensures result > 0
        {
            x + 1
        }";
        let sections = parse_function_sections(body, 0);

        assert!(sections.ensures_range.is_some());
    }

    #[test]
    fn test_parse_function_sections_with_both() {
        let body = "fn full_spec(x: i32) -> i32
            requires x > 0
            ensures result > x
        {
            x + 1
        }";
        let sections = parse_function_sections(body, 0);

        assert!(sections.requires_range.is_some());
        assert!(sections.ensures_range.is_some());
        assert!(sections.body_start_line.is_some());
    }

    // ==========================================================================
    // classify_call_location tests
    // ==========================================================================

    #[test]
    fn test_classify_call_location_inner() {
        let sections = FunctionSections {
            start_line: 0,
            requires_range: Some((1, 2)),
            ensures_range: Some((3, 4)),
            body_start_line: Some(5),
        };

        // Line 6 is inside the body
        assert_eq!(classify_call_location(6, &sections), CallLocation::Inner);
    }

    #[test]
    fn test_classify_call_location_precondition() {
        let sections = FunctionSections {
            start_line: 0,
            requires_range: Some((1, 2)),
            ensures_range: Some((3, 4)),
            body_start_line: Some(5),
        };

        assert_eq!(
            classify_call_location(1, &sections),
            CallLocation::Precondition
        );
        assert_eq!(
            classify_call_location(2, &sections),
            CallLocation::Precondition
        );
    }

    #[test]
    fn test_classify_call_location_postcondition() {
        let sections = FunctionSections {
            start_line: 0,
            requires_range: Some((1, 2)),
            ensures_range: Some((3, 4)),
            body_start_line: Some(5),
        };

        assert_eq!(
            classify_call_location(3, &sections),
            CallLocation::Postcondition
        );
        assert_eq!(
            classify_call_location(4, &sections),
            CallLocation::Postcondition
        );
    }

    #[test]
    fn test_classify_call_location_no_specs() {
        let sections = FunctionSections {
            start_line: 0,
            requires_range: None,
            ensures_range: None,
            body_start_line: Some(1),
        };

        // Everything should be Inner when there are no specs
        assert_eq!(classify_call_location(0, &sections), CallLocation::Inner);
        assert_eq!(classify_call_location(5, &sections), CallLocation::Inner);
    }

    // ==========================================================================
    // symbol_to_path tests
    // ==========================================================================

    #[test]
    fn test_symbol_to_path_basic() {
        let symbol = "rust-analyzer cargo my_crate 0.1.0 module/func().";
        let display_name = "func";
        let path = symbol_to_path(symbol, display_name);

        // Should produce a readable path ending with the display_name
        assert!(path.ends_with("func"));
        assert!(path.contains("module"));
    }

    #[test]
    fn test_symbol_to_path_with_generics() {
        let symbol = "rust-analyzer cargo lib 1.0.0 Container<T>/method().";
        let display_name = "method";
        let path = symbol_to_path(symbol, display_name);

        // Generics should be removed
        assert!(!path.contains("<T>"));
        assert!(path.ends_with("method"));
    }

    #[test]
    fn test_symbol_to_path_impl_block() {
        let symbol = "rust-analyzer cargo lib 1.0.0 impl#MyStruct/func().";
        let display_name = "func";
        let path = symbol_to_path(symbol, display_name);

        // impl# should be removed
        assert!(!path.contains("impl#"));
    }

    // ==========================================================================
    // generate_filtered_call_graph tests
    // ==========================================================================

    fn create_test_graph() -> HashMap<String, FunctionNode> {
        // Create a simple call graph: A -> B -> C -> D
        let mut graph = HashMap::new();

        let node_a = FunctionNode {
            symbol: "A".to_string(),
            display_name: "func_a".to_string(),
            file_path: "test.rs".to_string(),
            relative_path: "test.rs".to_string(),
            callers: HashSet::new(),
            callees: HashSet::from(["B".to_string()]),
            callee_occurrences: Vec::new(),
            range: vec![0],
            body: None,
        };

        let node_b = FunctionNode {
            symbol: "B".to_string(),
            display_name: "func_b".to_string(),
            file_path: "test.rs".to_string(),
            relative_path: "test.rs".to_string(),
            callers: HashSet::from(["A".to_string()]),
            callees: HashSet::from(["C".to_string()]),
            callee_occurrences: Vec::new(),
            range: vec![10],
            body: None,
        };

        let node_c = FunctionNode {
            symbol: "C".to_string(),
            display_name: "func_c".to_string(),
            file_path: "test.rs".to_string(),
            relative_path: "test.rs".to_string(),
            callers: HashSet::from(["B".to_string()]),
            callees: HashSet::from(["D".to_string()]),
            callee_occurrences: Vec::new(),
            range: vec![20],
            body: None,
        };

        let node_d = FunctionNode {
            symbol: "D".to_string(),
            display_name: "func_d".to_string(),
            file_path: "test.rs".to_string(),
            relative_path: "test.rs".to_string(),
            callers: HashSet::from(["C".to_string()]),
            callees: HashSet::new(),
            callee_occurrences: Vec::new(),
            range: vec![30],
            body: None,
        };

        graph.insert("A".to_string(), node_a);
        graph.insert("B".to_string(), node_b);
        graph.insert("C".to_string(), node_c);
        graph.insert("D".to_string(), node_d);

        graph
    }

    #[test]
    fn test_generate_filtered_call_graph_no_depth_limit() {
        let graph = create_test_graph();
        let filtered = generate_filtered_call_graph(&graph, &["A".to_string()], None);

        // Should include all nodes reachable from A
        assert_eq!(filtered.len(), 4);
        assert!(filtered.contains_key("A"));
        assert!(filtered.contains_key("B"));
        assert!(filtered.contains_key("C"));
        assert!(filtered.contains_key("D"));
    }

    #[test]
    fn test_generate_filtered_call_graph_with_depth_1() {
        let graph = create_test_graph();
        let filtered = generate_filtered_call_graph(&graph, &["A".to_string()], Some(1));

        // Should only include A and B (depth 1)
        assert_eq!(filtered.len(), 2);
        assert!(filtered.contains_key("A"));
        assert!(filtered.contains_key("B"));
        assert!(!filtered.contains_key("C"));
    }

    #[test]
    fn test_generate_filtered_call_graph_with_depth_2() {
        let graph = create_test_graph();
        let filtered = generate_filtered_call_graph(&graph, &["A".to_string()], Some(2));

        // Should include A, B, and C (depth 2)
        assert_eq!(filtered.len(), 3);
        assert!(filtered.contains_key("A"));
        assert!(filtered.contains_key("B"));
        assert!(filtered.contains_key("C"));
        assert!(!filtered.contains_key("D"));
    }

    #[test]
    fn test_generate_filtered_call_graph_nonexistent_entry() {
        let graph = create_test_graph();
        let filtered = generate_filtered_call_graph(&graph, &["NONEXISTENT".to_string()], None);

        // Should return empty graph
        assert!(filtered.is_empty());
    }

    #[test]
    fn test_generate_filtered_call_graph_multiple_entry_points() {
        let graph = create_test_graph();
        let filtered =
            generate_filtered_call_graph(&graph, &["A".to_string(), "C".to_string()], Some(1));

        // Should include A, B (from A) and C, D (from C)
        assert!(filtered.contains_key("A"));
        assert!(filtered.contains_key("B"));
        assert!(filtered.contains_key("C"));
        assert!(filtered.contains_key("D"));
    }
}
