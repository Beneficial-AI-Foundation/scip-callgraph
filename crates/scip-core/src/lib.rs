//! # scip-core
//!
//! Core library for parsing SCIP (Source Code Intelligence Protocol) data
//! and building call graphs from Rust projects.
//!
//! ## Architecture
//!
//! The library is organized into focused modules:
//!
//! - [`types`]: All shared data structures (SCIP types, graph types, D3 types)
//! - [`parser`]: SCIP JSON parsing utilities
//! - [`call_graph`]: Core call graph building and analysis
//! - [`export_d3`]: D3.js/web export functionality
//! - [`export_dot`]: DOT/Graphviz export for CLI visualization
//!
//! ## Additional Modules
//!
//! - [`scip_reader`]: Alternative SCIP file reader
//! - [`scip_utils`]: Utility functions for SCIP data manipulation
//! - [`call_graph_svg`]: Legacy SVG visualization
//! - [`atoms_to_d3`]: Convert probe-verus output to D3.js graph format
//! - [`logging`]: Logging utilities
//!
//! ## Quick Start
//!
//! ```ignore
//! use scip_core::{parse_scip_json, build_call_graph, export_call_graph_d3};
//!
//! let scip_data = parse_scip_json("index.scip.json")?;
//! let call_graph = build_call_graph(&scip_data);
//! export_call_graph_d3(&call_graph, &scip_data, "graph.json")?;
//! ```

// Core modules (new architecture)
pub mod call_graph;
pub mod export_d3;
pub mod export_dot;
pub mod parser;
pub mod types;

// Additional/legacy modules
pub mod atoms_to_d3;
pub mod call_graph_svg;
pub mod scip_reader;
pub mod scip_utils;

/// Logging utilities
pub mod logging {
    use log::LevelFilter;
    use std::env;

    /// Initialize logger based on debug flag or environment variable
    pub fn init_logger(debug: bool) {
        let log_level = if debug {
            LevelFilter::Debug
        } else if env::var("RUST_LOG").is_ok() {
            // Allow RUST_LOG to override if set
            env_logger::init();
            return;
        } else {
            LevelFilter::Warn
        };

        env_logger::Builder::new().filter_level(log_level).init();
    }

    /// Check if debug logging should be enabled from command line args
    pub fn should_enable_debug(args: &[String]) -> bool {
        args.iter().any(|arg| arg == "--debug" || arg == "-d")
    }
}

// Re-export commonly used types and functions for convenience
pub use call_graph::{
    build_call_graph, classify_call_location, detect_function_mode, generate_filtered_call_graph,
    is_function_like, parse_function_sections, print_call_graph_summary, symbol_to_path,
};
pub use export_d3::{export_call_graph_d3, write_call_graph_as_atoms_json};
pub use export_dot::{
    generate_call_graph_dot, generate_call_graph_dot_string, generate_call_graph_svg,
    generate_file_subgraph_dot, generate_files_subgraph_dot, generate_function_subgraph_dot,
    generate_svg_and_png_from_dot,
};
pub use parser::{
    extract_display_name_from_symbol, extract_path_info_from_symbol, parse_scip_json,
};
pub use types::{
    Atom, CallLocation, CalleeOccurrence, D3Graph, D3GraphMetadata, D3Link, D3Node, Document,
    FunctionMode, FunctionNode, FunctionSections, Metadata, Occurrence, ScipIndex,
    SignatureDocumentation, Symbol, ToolInfo,
};
