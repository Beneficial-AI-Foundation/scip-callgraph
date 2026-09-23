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
//
// These structs must tolerate missing fields: `scip print --json` uses proto3
// JSON serialization, which omits any field holding its default value (0,
// empty string, empty list, unset message). Every field that can legally be
// a protobuf default therefore carries #[serde(default)].

/// Root structure of a SCIP JSON index file
#[derive(Debug, Serialize, Deserialize)]
pub struct ScipIndex {
    pub metadata: Metadata,
    #[serde(default)]
    pub documents: Vec<Document>,
}

/// SCIP metadata about the indexed project
#[derive(Debug, Serialize, Deserialize)]
pub struct Metadata {
    #[serde(default)]
    pub tool_info: ToolInfo,
    #[serde(default)]
    pub project_root: String,
    #[serde(default)]
    pub text_document_encoding: i32,
}

/// Information about the tool that generated the SCIP index
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ToolInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
}

/// A document (source file) in the SCIP index
#[derive(Debug, Serialize, Deserialize)]
pub struct Document {
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub relative_path: String,
    #[serde(default)]
    pub occurrences: Vec<Occurrence>,
    #[serde(default)]
    pub symbols: Vec<Symbol>,
    #[serde(default)]
    pub position_encoding: i32,
}

/// An occurrence of a symbol in the source code
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Occurrence {
    #[serde(default)]
    pub range: Vec<i32>,
    #[serde(default)]
    pub symbol: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_roles: Option<i32>,
}

/// A symbol definition in the SCIP index
#[derive(Debug, Serialize, Deserialize)]
pub struct Symbol {
    #[serde(default)]
    pub symbol: String,
    #[serde(default)]
    pub kind: i32,
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation: Option<Vec<String>>,
    #[serde(default)]
    pub signature_documentation: SignatureDocumentation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enclosing_symbol: Option<String>,
}

/// Signature documentation for a symbol
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SignatureDocumentation {
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
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

/// Declaration kind (Verus: exec/proof/spec, Lean: theorem/def/axiom/...)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeclKind {
    /// Executable code (default Rust functions)
    Exec,
    /// Proof functions (lemmas, verification helpers)
    Proof,
    /// Specification functions (pure mathematical definitions)
    Spec,
}

impl DeclKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            DeclKind::Exec => "exec",
            DeclKind::Proof => "proof",
            DeclKind::Spec => "spec",
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
    /// Declaration kind: exec, proof, or spec
    pub kind: DeclKind,
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
    // DeclKind tests
    // ==========================================================================

    #[test]
    fn test_decl_kind_as_str() {
        assert_eq!(DeclKind::Exec.as_str(), "exec");
        assert_eq!(DeclKind::Proof.as_str(), "proof");
        assert_eq!(DeclKind::Spec.as_str(), "spec");
    }

    #[test]
    fn test_decl_kind_serialization() {
        let exec_json = serde_json::to_string(&DeclKind::Exec).unwrap();
        let proof_json = serde_json::to_string(&DeclKind::Proof).unwrap();
        let spec_json = serde_json::to_string(&DeclKind::Spec).unwrap();

        assert_eq!(exec_json, "\"exec\"");
        assert_eq!(proof_json, "\"proof\"");
        assert_eq!(spec_json, "\"spec\"");
    }

    #[test]
    fn test_decl_kind_deserialization() {
        let exec: DeclKind = serde_json::from_str("\"exec\"").unwrap();
        let proof: DeclKind = serde_json::from_str("\"proof\"").unwrap();
        let spec: DeclKind = serde_json::from_str("\"spec\"").unwrap();

        assert_eq!(exec, DeclKind::Exec);
        assert_eq!(proof, DeclKind::Proof);
        assert_eq!(spec, DeclKind::Spec);
    }

    // ==========================================================================
    // SCIP proto3 JSON tests — fields at their protobuf default are omitted
    // ==========================================================================

    // Shape emitted by scip CLI >= v0.9: signature_documentation without
    // position_encoding, symbols without kind/display_name, plus unknown
    // TypedRange keys (regression test for the downstream pipeline break).
    #[test]
    fn test_scip_index_parses_proto3_json_with_omitted_defaults() {
        let json = r#"{
            "metadata": {
                "tool_info": {"name": "rust-analyzer", "version": "0.3.2593-standalone"},
                "project_root": "file:///tmp/project"
            },
            "documents": [{
                "language": "rust",
                "relative_path": "src/lib.rs",
                "occurrences": [
                    {"range": [3, 7, 16], "TypedRange": null, "symbol": "rust-analyzer cargo quicksort 0.1.0 quicksort().", "symbol_roles": 1, "TypedEnclosingRange": null}
                ],
                "symbols": [
                    {"symbol": "rust-analyzer cargo quicksort 0.1.0 crate/", "signature_documentation": {"language": "rust", "text": "extern crate quicksort"}}
                ],
                "position_encoding": 1
            }]
        }"#;

        let index: ScipIndex = serde_json::from_str(json).unwrap();

        assert_eq!(index.metadata.text_document_encoding, 0);
        assert_eq!(index.documents.len(), 1);
        let doc = &index.documents[0];
        assert_eq!(doc.occurrences.len(), 1);
        let sym = &doc.symbols[0];
        assert_eq!(sym.kind, 0);
        assert_eq!(sym.display_name, None);
        assert_eq!(sym.signature_documentation.position_encoding, 0);
    }

    #[test]
    fn test_scip_index_parses_minimal_document() {
        // A document with no occurrences/symbols/position_encoding at all.
        let json = r#"{
            "metadata": {"tool_info": {"name": "x", "version": "1"}, "project_root": "file:///p", "text_document_encoding": 1},
            "documents": [{"language": "rust", "relative_path": "src/empty.rs"}]
        }"#;

        let index: ScipIndex = serde_json::from_str(json).unwrap();
        let doc = &index.documents[0];
        assert!(doc.occurrences.is_empty());
        assert!(doc.symbols.is_empty());
        assert_eq!(doc.position_encoding, 0);
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
            kind: DeclKind::Exec,
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
            kind: DeclKind::Exec,
        };

        let json = serde_json::to_string(&node).unwrap();

        assert!(json.contains("\"start_line\":10"));
        assert!(json.contains("\"end_line\":20"));
    }
}
