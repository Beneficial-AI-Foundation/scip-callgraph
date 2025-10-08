//! This is the main documentation for the scip-callgraph crate.
// Re-export the scip_call_graph module
pub mod scip_call_graph;

pub mod scip_reader;

pub mod scip_to_call_graph_json;

/// Logging utilities
pub mod logging {
    use std::env;
    use log::LevelFilter;

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

        env_logger::Builder::new()
            .filter_level(log_level)
            .init();
    }

    /// Check if debug logging should be enabled from command line args
    pub fn should_enable_debug(args: &[String]) -> bool {
        args.iter().any(|arg| arg == "--debug" || arg == "-d")
    }
}
