//! Shared types for SCIP call graph analysis.
//!
//! This module contains all the data structures used across the scip-core library:
//! - SCIP index types (from SCIP JSON format)
//! - Call graph types (nodes, edges, occurrences)
//! - D3.js export types (for web visualization)
//! - Verus-specific types (function modes, sections)

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

// =============================================================================
// SCIP Index Types (from SCIP JSON format)
// =============================================================================

/// Root structure of a SCIP JSON index file
#[derive(Debug, Serialize, Deserialize)]
pub struct ScipIndex {
    pub metadata: Metadata,
    pub documents: Vec<Document>,
}

/// SCIP metadata about the indexed project
#[derive(Debug, Serialize, Deserialize)]
pub struct Metadata {
    pub tool_info: ToolInfo,
    pub project_root: String,
    pub text_document_encoding: i32,
}

/// Information about the tool that generated the SCIP index
#[derive(Debug, Serialize, Deserialize)]
pub struct ToolInfo {
    pub name: String,
    pub version: String,
}

/// A document (source file) in the SCIP index
#[derive(Debug, Serialize, Deserialize)]
pub struct Document {
    pub language: String,
    pub relative_path: String,
    pub occurrences: Vec<Occurrence>,
    #[serde(default)]
    pub symbols: Vec<Symbol>,
    pub position_encoding: i32,
}

/// An occurrence of a symbol in the source code
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Occurrence {
    pub range: Vec<i32>,
    pub symbol: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_roles: Option<i32>,
}

/// A symbol definition in the SCIP index
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

/// Signature documentation for a symbol
#[derive(Debug, Serialize, Deserialize)]
pub struct SignatureDocumentation {
    pub language: String,
    pub text: String,
    pub position_encoding: i32,
}

// =============================================================================
// Call Graph Types
// =============================================================================

/// Represents where a function call occurs within its caller
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CallLocation {
    /// Call occurs in a `requires` clause (precondition)
    Precondition,
    /// Call occurs in an `ensures` clause (postcondition)
    Postcondition,
    /// Call occurs in the function body (after opening brace)
    Inner,
}

impl CallLocation {
    pub fn as_str(&self) -> &'static str {
        match self {
            CallLocation::Precondition => "precondition",
            CallLocation::Postcondition => "postcondition",
            CallLocation::Inner => "inner",
        }
    }
}

/// A callee occurrence with its location information
#[derive(Debug, Clone)]
pub struct CalleeOccurrence {
    pub symbol: String,
    pub line: i32,
    pub location: Option<CallLocation>,
}

/// Represents a node in the call graph
#[derive(Debug, Clone)]
pub struct FunctionNode {
    pub symbol: String,
    pub display_name: String,
    pub file_path: String,
    pub relative_path: String,
    pub callers: HashSet<String>,
    pub callees: HashSet<String>,
    pub callee_occurrences: Vec<CalleeOccurrence>,
    pub range: Vec<i32>,
    pub body: Option<String>,
}

/// An atom represents a function with its dependencies (for JSON export)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Atom {
    pub identifier: String,
    pub statement_type: String,
    pub deps: Vec<String>,
    pub body: String,
    pub display_name: String,
    pub full_path: String,
    pub relative_path: String,
    pub file_name: String,
    pub parent_folder: String,
}

// =============================================================================
// Verus-Specific Types
// =============================================================================

/// Verus function modes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FunctionMode {
    /// Executable code (default Rust functions)
    Exec,
    /// Proof functions (lemmas, verification helpers)
    Proof,
    /// Specification functions (pure mathematical definitions)
    Spec,
}

impl FunctionMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            FunctionMode::Exec => "exec",
            FunctionMode::Proof => "proof",
            FunctionMode::Spec => "spec",
        }
    }
}

/// Represents the line ranges of different sections in a Verus function
#[derive(Debug, Clone, Default)]
pub struct FunctionSections {
    /// Line number where the function starts (0-based, as stored in SCIP)
    pub start_line: i32,
    /// Line range for `requires` clause (start, end) - 0-based
    pub requires_range: Option<(i32, i32)>,
    /// Line range for `ensures` clause (start, end) - 0-based
    pub ensures_range: Option<(i32, i32)>,
    /// Line number where the function body starts (the `{`) - 0-based
    pub body_start_line: Option<i32>,
}

// =============================================================================
// D3.js Export Types (for web visualization)
// =============================================================================

/// A node in the D3.js force-directed graph
#[derive(Debug, Serialize, Deserialize)]
pub struct D3Node {
    pub id: String,
    pub display_name: String,
    pub symbol: String,
    pub full_path: String,
    pub relative_path: String,
    pub file_name: String,
    pub parent_folder: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_line: Option<usize>,
    pub is_libsignal: bool,
    /// Functions this function calls (outgoing edges) - scip_names for O(1) lookup
    pub dependencies: Vec<String>,
    /// Functions that call this function (incoming edges) - scip_names for O(1) lookup
    pub dependents: Vec<String>,
    /// Verus function mode: exec, proof, or spec
    pub mode: FunctionMode,
}

/// A link (edge) in the D3.js graph
#[derive(Debug, Serialize, Deserialize)]
pub struct D3Link {
    pub source: String,
    pub target: String,
    #[serde(rename = "type")]
    pub link_type: String,
}

/// Metadata for the D3.js graph
#[derive(Debug, Serialize, Deserialize)]
pub struct D3GraphMetadata {
    pub total_nodes: usize,
    pub total_edges: usize,
    pub project_root: String,
    pub generated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github_url: Option<String>,
}

/// Complete D3.js graph structure
#[derive(Debug, Serialize, Deserialize)]
pub struct D3Graph {
    pub nodes: Vec<D3Node>,
    pub links: Vec<D3Link>,
    pub metadata: D3GraphMetadata,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==========================================================================
    // FunctionMode tests
    // ==========================================================================

    #[test]
    fn test_function_mode_as_str() {
        assert_eq!(FunctionMode::Exec.as_str(), "exec");
        assert_eq!(FunctionMode::Proof.as_str(), "proof");
        assert_eq!(FunctionMode::Spec.as_str(), "spec");
    }

    #[test]
    fn test_function_mode_serialization() {
        // FunctionMode should serialize to lowercase strings
        let exec_json = serde_json::to_string(&FunctionMode::Exec).unwrap();
        let proof_json = serde_json::to_string(&FunctionMode::Proof).unwrap();
        let spec_json = serde_json::to_string(&FunctionMode::Spec).unwrap();
        
        assert_eq!(exec_json, "\"exec\"");
        assert_eq!(proof_json, "\"proof\"");
        assert_eq!(spec_json, "\"spec\"");
    }

    #[test]
    fn test_function_mode_deserialization() {
        let exec: FunctionMode = serde_json::from_str("\"exec\"").unwrap();
        let proof: FunctionMode = serde_json::from_str("\"proof\"").unwrap();
        let spec: FunctionMode = serde_json::from_str("\"spec\"").unwrap();
        
        assert_eq!(exec, FunctionMode::Exec);
        assert_eq!(proof, FunctionMode::Proof);
        assert_eq!(spec, FunctionMode::Spec);
    }

    // ==========================================================================
    // CallLocation tests
    // ==========================================================================

    #[test]
    fn test_call_location_as_str() {
        assert_eq!(CallLocation::Precondition.as_str(), "precondition");
        assert_eq!(CallLocation::Postcondition.as_str(), "postcondition");
        assert_eq!(CallLocation::Inner.as_str(), "inner");
    }

    // ==========================================================================
    // D3Link serialization tests - link_type field renaming
    // ==========================================================================

    #[test]
    fn test_d3_link_serialization_renames_type() {
        let link = D3Link {
            source: "a".to_string(),
            target: "b".to_string(),
            link_type: "inner".to_string(),
        };
        
        let json = serde_json::to_string(&link).unwrap();
        
        // Should serialize as "type" not "link_type"
        assert!(json.contains("\"type\""));
        assert!(!json.contains("\"link_type\""));
    }

    #[test]
    fn test_d3_link_deserialization_from_type() {
        let json = r#"{"source":"a","target":"b","type":"precondition"}"#;
        let link: D3Link = serde_json::from_str(json).unwrap();
        
        assert_eq!(link.source, "a");
        assert_eq!(link.target, "b");
        assert_eq!(link.link_type, "precondition");
    }

    // ==========================================================================
    // Atom serialization tests
    // ==========================================================================

    #[test]
    fn test_atom_roundtrip_serialization() {
        let atom = Atom {
            identifier: "my_crate::my_func".to_string(),
            statement_type: "function".to_string(),
            deps: vec!["dep1".to_string(), "dep2".to_string()],
            body: "fn my_func() { }".to_string(),
            display_name: "my_func".to_string(),
            full_path: "/path/to/file.rs".to_string(),
            relative_path: "src/file.rs".to_string(),
            file_name: "file.rs".to_string(),
            parent_folder: "src".to_string(),
        };
        
        let json = serde_json::to_string(&atom).unwrap();
        let parsed: Atom = serde_json::from_str(&json).unwrap();
        
        assert_eq!(parsed.identifier, atom.identifier);
        assert_eq!(parsed.deps.len(), 2);
        assert_eq!(parsed.display_name, atom.display_name);
    }

    // ==========================================================================
    // D3Node optional fields tests
    // ==========================================================================

    #[test]
    fn test_d3_node_optional_fields_skipped_when_none() {
        let node = D3Node {
            id: "test".to_string(),
            display_name: "test".to_string(),
            symbol: "test".to_string(),
            full_path: "/test".to_string(),
            relative_path: "test".to_string(),
            file_name: "test.rs".to_string(),
            parent_folder: "src".to_string(),
            start_line: None,
            end_line: None,
            is_libsignal: false,
            dependencies: vec![],
            dependents: vec![],
            mode: FunctionMode::Exec,
        };
        
        let json = serde_json::to_string(&node).unwrap();
        
        // start_line and end_line should not appear in JSON when None
        assert!(!json.contains("start_line"));
        assert!(!json.contains("end_line"));
    }

    #[test]
    fn test_d3_node_optional_fields_included_when_some() {
        let node = D3Node {
            id: "test".to_string(),
            display_name: "test".to_string(),
            symbol: "test".to_string(),
            full_path: "/test".to_string(),
            relative_path: "test".to_string(),
            file_name: "test.rs".to_string(),
            parent_folder: "src".to_string(),
            start_line: Some(10),
            end_line: Some(20),
            is_libsignal: false,
            dependencies: vec![],
            dependents: vec![],
            mode: FunctionMode::Exec,
        };
        
        let json = serde_json::to_string(&node).unwrap();
        
        assert!(json.contains("\"start_line\":10"));
        assert!(json.contains("\"end_line\":20"));
    }
}

