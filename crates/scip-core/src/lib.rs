//! # scip-core
//!
//! Core library for parsing SCIP (Source Code Intelligence Protocol) data
//! and building call graphs from Rust projects.
//!
//! ## Modules
//!
//! - [`scip_reader`]: Read and parse SCIP index files
//! - [`scip_to_call_graph_json`]: Convert SCIP data to call graph JSON format
//! - [`scip_call_graph`]: Call graph data structures and operations
//! - [`scip_utils`]: Utility functions for SCIP data manipulation
//! - [`call_graph_svg`]: SVG visualization of call graphs
//! - [`atoms_to_d3`]: Convert scip-atoms output to D3.js graph format
//! - [`logging`]: Logging utilities

pub mod atoms_to_d3;
pub mod call_graph_svg;
pub mod scip_call_graph;
pub mod scip_reader;
pub mod scip_to_call_graph_json;
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

// Re-export commonly used types
pub use scip_to_call_graph_json::{
    build_call_graph, parse_scip_json, write_call_graph_as_atoms_json, FunctionNode,
};
