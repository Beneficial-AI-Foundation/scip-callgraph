//! Convert probe-verus output to D3.js graph format.
//!
//! This module provides a converter that takes probe-verus' `AtomWithLines` output
//! and enriches it with additional visualization data for the D3.js web viewer.
//!
//! This approach keeps probe-verus unchanged while extending its output for our needs.

use crate::types::{D3Graph, D3GraphMetadata, D3Link, D3Node, FunctionMode};
use probe_verus::{AtomWithLines, CallLocation, FunctionNode};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Convert probe-verus' call graph and atoms to D3Graph format.
///
/// This function takes:
/// - `atoms`: The HashMap of scip_name -> AtomWithLines from probe-verus
/// - `call_graph`: The original FunctionNode map (unused, kept for API compatibility)
/// - `project_root`: The project root path for metadata
/// - `github_url`: Optional GitHub URL for source links
///
/// Returns a D3Graph suitable for the web viewer with pre-computed dependencies/dependents
/// for O(1) lookups in the browser.
pub fn atoms_to_d3_graph(
    atoms: &HashMap<String, AtomWithLines>,
    _call_graph: &HashMap<String, FunctionNode>,
    project_root: &str,
    github_url: Option<String>,
) -> D3Graph {
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

            // Get mode directly from atom (parsed by probe-verus using verus_syn)
            let mode = mode_from_string(&atom.mode);

            // Check if this is a libsignal node (based on path patterns)
            let is_libsignal =
                is_libsignal_path(&atom.code_path) || atom.scip_name.contains("libsignal");

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

    // Convert dependencies to D3Links with proper call location types
    // Uses dependencies_with_locations which tracks where each call occurs
    // (precondition/postcondition/inner)
    let mut link_set: HashSet<(String, String, String)> = HashSet::new();
    let links: Vec<D3Link> = atoms
        .values()
        .flat_map(|atom| {
            atom.dependencies_with_locations
                .iter()
                .filter_map(|dep| {
                    // Only create link if the target exists in atoms
                    if atoms.contains_key(&dep.scip_name) {
                        let link_type = match dep.location {
                            CallLocation::Precondition => "precondition",
                            CallLocation::Postcondition => "postcondition",
                            CallLocation::Inner => "inner",
                        }
                        .to_string();

                        // Deduplicate links (same source, target, type)
                        let key = (
                            atom.scip_name.clone(),
                            dep.scip_name.clone(),
                            link_type.clone(),
                        );
                        if link_set.insert(key) {
                            Some(D3Link {
                                source: atom.scip_name.clone(),
                                target: dep.scip_name.clone(),
                                link_type,
                            })
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
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

/// Convert a mode string from probe-verus to FunctionMode enum.
fn mode_from_string(mode: &str) -> FunctionMode {
    match mode {
        "spec" => FunctionMode::Spec,
        "proof" => FunctionMode::Proof,
        _ => FunctionMode::Exec,
    }
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
    fn test_mode_from_string() {
        assert_eq!(mode_from_string("exec"), FunctionMode::Exec);
        assert_eq!(mode_from_string("proof"), FunctionMode::Proof);
        assert_eq!(mode_from_string("spec"), FunctionMode::Spec);
        // Unknown modes default to exec
        assert_eq!(mode_from_string("unknown"), FunctionMode::Exec);
        assert_eq!(mode_from_string(""), FunctionMode::Exec);
    }
}
