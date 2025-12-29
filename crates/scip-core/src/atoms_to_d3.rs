//! Convert scip-atoms output to D3.js graph format.
//!
//! This module provides a converter that takes scip-atoms' `AtomWithLines` output
//! and enriches it with additional visualization data for the D3.js web viewer.
//!
//! This approach keeps scip-atoms unchanged while extending its output for our needs.

use crate::types::{D3Graph, D3GraphMetadata, D3Link, D3Node, FunctionMode};
use scip_atoms::{AtomWithLines, FunctionNode};
use std::collections::HashMap;
use std::path::Path;

/// Convert scip-atoms' call graph and atoms to D3Graph format.
///
/// This function takes:
/// - `atoms`: The HashMap of scip_name -> AtomWithLines from scip-atoms
/// - `call_graph`: The original FunctionNode map (for signature_text to detect mode)
/// - `project_root`: The project root path for metadata
/// - `github_url`: Optional GitHub URL for source links
///
/// Returns a D3Graph suitable for the web viewer with pre-computed dependencies/dependents
/// for O(1) lookups in the browser.
pub fn atoms_to_d3_graph(
    atoms: &HashMap<String, AtomWithLines>,
    call_graph: &HashMap<String, FunctionNode>,
    project_root: &str,
    github_url: Option<String>,
) -> D3Graph {
    // Build a lookup from scip_name to FunctionNode for mode detection
    // The key in call_graph is the unique_key (symbol|signature|self_type@line)
    // We need to match by display_name + relative_path
    let mut node_lookup: HashMap<(&str, &str), &FunctionNode> = HashMap::new();
    for node in call_graph.values() {
        node_lookup.insert((&node.display_name, &node.relative_path), node);
    }

    // Build dependents map: for each scip_name, collect all scip_names that depend on it
    // This is the inverse of dependencies - enables O(1) "who calls me" lookups
    let mut dependents_map: HashMap<&str, Vec<String>> = HashMap::new();
    for atom in atoms.values() {
        for dep in &atom.dependencies {
            // Only add if the dependency exists in atoms (filter out external deps)
            if atoms.contains_key(dep) {
                dependents_map
                    .entry(dep.as_str())
                    .or_default()
                    .push(atom.scip_name.clone());
            }
        }
    }

    // Convert atoms to D3Nodes
    let nodes: Vec<D3Node> = atoms
        .values()
        .map(|atom| {
            // Extract file info from code_path
            let path = Path::new(&atom.code_path);
            let file_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();
            let parent_folder = path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            // Detect mode from signature_text if we have the original FunctionNode
            let mode = node_lookup
                .get(&(atom.display_name.as_str(), atom.code_path.as_str()))
                .map(|node| detect_function_mode(&node.signature_text))
                .unwrap_or(FunctionMode::Exec);

            // Check if this is a libsignal node (based on path patterns)
            let is_libsignal = is_libsignal_path(&atom.code_path) 
                || atom.scip_name.contains("libsignal");

            // Filter dependencies to only include those that exist in atoms
            let dependencies: Vec<String> = atom
                .dependencies
                .iter()
                .filter(|dep| atoms.contains_key(*dep))
                .cloned()
                .collect();

            // Get dependents (who calls this function)
            let dependents = dependents_map
                .get(atom.scip_name.as_str())
                .cloned()
                .unwrap_or_default();

            D3Node {
                id: atom.scip_name.clone(),
                display_name: atom.display_name.clone(),
                symbol: atom.scip_name.clone(), // Use scip_name as symbol for consistency
                full_path: atom.code_path.clone(), // Use relative path (full_path is legacy)
                relative_path: atom.code_path.clone(),
                file_name,
                parent_folder,
                start_line: Some(atom.code_text.lines_start),
                end_line: Some(atom.code_text.lines_end),
                is_libsignal,
                dependencies,
                dependents,
                mode,
            }
        })
        .collect();

    // Convert dependencies to D3Links
    // Note: scip-atoms doesn't distinguish call location (inner/precondition/postcondition)
    // so all links are "inner" by default
    let links: Vec<D3Link> = atoms
        .values()
        .flat_map(|atom| {
            atom.dependencies.iter().filter_map(|dep_scip_name| {
                // Only create link if the target exists in atoms
                if atoms.contains_key(dep_scip_name) {
                    Some(D3Link {
                        source: atom.scip_name.clone(),
                        target: dep_scip_name.clone(),
                        link_type: "inner".to_string(),
                    })
                } else {
                    None
                }
            })
        })
        .collect();

    // Create metadata
    let now = chrono::Utc::now();
    let metadata = D3GraphMetadata {
        total_nodes: nodes.len(),
        total_edges: links.len(),
        project_root: project_root.to_string(),
        generated_at: now.to_rfc3339(),
        github_url,
    };

    D3Graph {
        nodes,
        links,
        metadata,
    }
}

/// Detect the Verus function mode from a function signature text.
fn detect_function_mode(signature: &str) -> FunctionMode {
    let signature_lower = signature.to_lowercase();

    // Check for spec functions (including open/closed spec)
    if signature_lower.contains("spec fn")
        || signature_lower.contains("spec(checked) fn")
        || signature_lower.contains("open spec fn")
        || signature_lower.contains("closed spec fn")
    {
        return FunctionMode::Spec;
    }

    // Check for proof functions
    if signature_lower.contains("proof fn") {
        return FunctionMode::Proof;
    }

    // Default to exec (regular executable functions)
    FunctionMode::Exec
}

/// Check if a path belongs to the libsignal project.
fn is_libsignal_path(path: &str) -> bool {
    path.contains("libsignal")
        || path.contains("zkgroup")
        || path.contains("poksho")
        || path.contains("zkcredential")
        || path.contains("usernames")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_function_mode() {
        assert_eq!(detect_function_mode("fn foo()"), FunctionMode::Exec);
        assert_eq!(detect_function_mode("pub fn bar()"), FunctionMode::Exec);
        assert_eq!(detect_function_mode("proof fn lemma()"), FunctionMode::Proof);
        assert_eq!(detect_function_mode("spec fn invariant()"), FunctionMode::Spec);
        assert_eq!(detect_function_mode("open spec fn predicate()"), FunctionMode::Spec);
    }
}

