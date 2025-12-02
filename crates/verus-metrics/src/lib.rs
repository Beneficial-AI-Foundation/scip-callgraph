//! # verus-metrics
//!
//! Library for computing Halstead complexity metrics for Verus specifications and proofs.
//!
//! ## Features
//!
//! - Parse Verus `requires`, `ensures`, and `decreases` clauses
//! - Compute Halstead metrics (n1, N1, n2, N2, length, difficulty, effort, etc.)
//! - Handle Verus-specific syntax (quantifiers, implications, ghost operators)
//!
//! ## Example
//!
//! ```rust,ignore
//! use verus_metrics::spec_halstead::analyze_spec;
//!
//! let spec = "x > 0 && y < 100";
//! let metrics = analyze_spec(spec);
//! println!("Halstead length: {:?}", metrics.halstead_length);
//! ```

pub mod spec_halstead;

// Re-export main types
pub use spec_halstead::{analyze_spec, is_prose, SpecHalsteadMetrics};
