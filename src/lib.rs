//! This is the main documentation for the rust-analyzer-test crate.

// Add modules
pub mod helper;
// Re-export the scip_call_graph module
pub mod scip_call_graph;

pub mod scip_reader;

/// Performs a simple calculation
/// 
/// # Examples
/// ```
/// let result = rust_analyzer_test::calculate(2, 3);
/// assert_eq!(result, 5);
/// ```
pub fn calculate(a: i32, b: i32) -> i32 {
    a + b
}

/// A simple data structure
#[derive(Debug)]
pub struct Data {
    /// The name field
    pub name: String,
    /// The value field
    pub value: i32,
}

impl Data {
    /// Creates a new Data instance
    pub fn new(name: &str, value: i32) -> Self {
        Self {
            name: name.to_string(),
            value,
        }
    }
}
