//! SCIP JSON parsing utilities.
//!
//! This module provides functions to parse SCIP (Source Code Intelligence Protocol)
//! JSON index files into structured Rust types.

use crate::types::ScipIndex;
use std::fs;
use std::path::Path;

/// Parse a SCIP JSON file into a ScipIndex structure.
///
/// # Arguments
/// * `file_path` - Path to the SCIP JSON file
///
/// # Returns
/// * `Ok(ScipIndex)` - The parsed SCIP index
/// * `Err` - If the file cannot be read or parsed
///
/// # Example
/// ```ignore
/// use scip_core::parser::parse_scip_json;
///
/// let scip_data = parse_scip_json("index.scip.json")?;
/// println!("Project root: {}", scip_data.metadata.project_root);
/// ```
pub fn parse_scip_json(file_path: &str) -> Result<ScipIndex, Box<dyn std::error::Error>> {
    let path = Path::new(file_path);
    let contents = fs::read_to_string(path)?;
    let index: ScipIndex = serde_json::from_str(&contents)?;
    Ok(index)
}

/// Extract display name from a SCIP symbol string.
///
/// SCIP symbols have a structured format like:
/// `rust-analyzer cargo dalek_test 0.1.0 module/function_name().`
///
/// This function extracts the human-readable function name from the symbol.
pub fn extract_display_name_from_symbol(symbol: &str) -> String {
    // Extract the last part of the symbol path
    // Format: "rust-analyzer cargo dalek_test 0.1.0 module/submodule/function_name()."
    let parts: Vec<&str> = symbol.split(' ').collect();
    if parts.len() < 5 {
        return symbol.to_string();
    }

    // Get the path part (after the first 4 parts: tool, manager, crate, version)
    let path_part = parts[4..].join(" ");

    // Extract just the function name (last component before the parentheses)
    if let Some(last_slash) = path_part.rfind('/') {
        let name = &path_part[last_slash + 1..];
        // Remove trailing (). and any method markers
        name.trim_end_matches('.')
            .trim_end_matches("()")
            .to_string()
    } else {
        path_part
            .trim_end_matches('.')
            .trim_end_matches("()")
            .to_string()
    }
}

/// Extract path components from a SCIP symbol string.
///
/// Returns a tuple of (full_path, file_name, parent_folder).
///
/// # Arguments
/// * `symbol` - The SCIP symbol string
///
/// # Returns
/// * `(full_path, file_name, parent_folder)` tuple
pub fn extract_path_info_from_symbol(symbol: &str) -> (String, String, String) {
    // Parse symbol format: "rust-analyzer cargo crate_name version path/to/module/function()."
    let parts: Vec<&str> = symbol.split(' ').collect();
    if parts.len() < 5 {
        return (symbol.to_string(), String::new(), String::new());
    }

    // Get crate name and path
    let crate_name = parts[2];
    let path_part = parts[4..].join(" ");

    // Build full path: crate_name::module::submodule::function
    let path_components: Vec<&str> = path_part
        .trim_end_matches('.')
        .trim_end_matches("()")
        .split('/')
        .collect();

    let full_path = format!("{}::{}", crate_name, path_components.join("::"));

    // Extract file_name (last component) and parent_folder
    let file_name = path_components.last().unwrap_or(&"").to_string();
    let parent_folder = if path_components.len() > 1 {
        path_components[..path_components.len() - 1].join("::")
    } else {
        crate_name.to_string()
    };

    (full_path, file_name, parent_folder)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==========================================================================
    // extract_display_name_from_symbol tests
    // ==========================================================================

    #[test]
    fn test_extract_display_name_basic() {
        let symbol = "rust-analyzer cargo my_crate 0.1.0 module/submodule/my_function().";
        assert_eq!(extract_display_name_from_symbol(symbol), "my_function");
    }

    #[test]
    fn test_extract_display_name_nested_module() {
        // Note: impl method symbols include the struct name with #
        let symbol = "rust-analyzer cargo dalek 4.1.0 backend/serial/curve_models/AffineNielsPoint#neg().";
        // The function returns "StructName#method" for impl blocks, preserving context
        assert_eq!(extract_display_name_from_symbol(symbol), "AffineNielsPoint#neg");
    }

    #[test]
    fn test_extract_display_name_impl_block() {
        let symbol = "rust-analyzer cargo curve25519 0.1.0 field/FieldElement51#square().";
        // The function returns "StructName#method" for impl blocks
        assert_eq!(extract_display_name_from_symbol(symbol), "FieldElement51#square");
    }

    #[test]
    fn test_extract_display_name_short_symbol_returns_as_is() {
        // Symbols with fewer than 5 parts should return as-is
        let symbol = "short symbol";
        assert_eq!(extract_display_name_from_symbol(symbol), "short symbol");
    }

    #[test]
    fn test_extract_display_name_no_slash() {
        let symbol = "rust-analyzer cargo crate 0.1.0 simple_function().";
        assert_eq!(extract_display_name_from_symbol(symbol), "simple_function");
    }

    // ==========================================================================
    // extract_path_info_from_symbol tests
    // ==========================================================================

    #[test]
    fn test_extract_path_info_basic() {
        let symbol = "rust-analyzer cargo my_crate 0.1.0 module/submodule/my_function().";
        let (full, name, parent) = extract_path_info_from_symbol(symbol);
        assert_eq!(full, "my_crate::module::submodule::my_function");
        assert_eq!(name, "my_function");
        assert_eq!(parent, "module::submodule");
    }

    #[test]
    fn test_extract_path_info_single_module() {
        let symbol = "rust-analyzer cargo my_crate 0.1.0 my_function().";
        let (full, name, parent) = extract_path_info_from_symbol(symbol);
        assert_eq!(full, "my_crate::my_function");
        assert_eq!(name, "my_function");
        assert_eq!(parent, "my_crate"); // Falls back to crate name
    }

    #[test]
    fn test_extract_path_info_deep_nesting() {
        let symbol = "rust-analyzer cargo lib 1.0.0 a/b/c/d/func().";
        let (full, name, parent) = extract_path_info_from_symbol(symbol);
        assert_eq!(full, "lib::a::b::c::d::func");
        assert_eq!(name, "func");
        assert_eq!(parent, "a::b::c::d");
    }

    #[test]
    fn test_extract_path_info_short_symbol() {
        let symbol = "too short";
        let (full, name, parent) = extract_path_info_from_symbol(symbol);
        assert_eq!(full, "too short");
        assert_eq!(name, "");
        assert_eq!(parent, "");
    }
}

