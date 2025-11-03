//! Utilities for working with SCIP (Source Code Indexing Protocol)

use std::path::Path;
use std::process::Command;

/// Generate SCIP JSON index for a given folder path
///
/// This function runs `verus-analyzer scip <folder_path>` followed by
/// `scip print --json index.scip` and writes the output to a JSON file
/// named `<folder_name>_index_scip.json`.
///
/// # Arguments
///
/// * `folder_path` - Path to the folder to analyze
///
/// # Returns
///
/// Returns the path to the generated JSON file on success, or an error on failure.
///
/// # Example
///
/// ```no_run
/// use scip_callgraph::scip_utils::generate_scip_json_index;
///
/// let output_file = generate_scip_json_index("/path/to/project").unwrap();
/// println!("Generated: {}", output_file);
/// ```
pub fn generate_scip_json_index(folder_path: &str) -> Result<String, Box<dyn std::error::Error>> {
    let folder_path = Path::new(folder_path);

    // Validate that the folder exists
    if !folder_path.exists() {
        return Err(format!("Path '{}' does not exist", folder_path.display()).into());
    }

    if !folder_path.is_dir() {
        return Err(format!("Path '{}' is not a directory", folder_path.display()).into());
    }

    println!("Running verus-analyzer scip on '{}'...", folder_path.display());

    // Step 1: Run verus-analyzer scip <path_to_folder>
    let status = Command::new("verus-analyzer")
        .arg("scip")
        .arg(folder_path)
        .status()?;

    if !status.success() {
        return Err(format!("verus-analyzer scip command failed with status: {}", status).into());
    }

    println!("verus-analyzer scip completed successfully");

    // Step 2: Determine output filename
    // Extract the folder name to use in the output filename
    let folder_name = folder_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("output");
    let output_filename = format!("{}_index_scip.json", folder_name);

    println!("Generating SCIP JSON output at '{}'...", output_filename);

    // Step 3: Run scip print --json index.scip > <path_to_folder>_index_scip.json
    let output = Command::new("scip")
        .args(["print", "--json", "index.scip"])
        .output()?;

    if !output.status.success() {
        return Err(format!(
            "scip print command failed with status: {}\nstderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    // Write the output to the JSON file
    std::fs::write(&output_filename, output.stdout)?;

    let file_size = std::fs::metadata(&output_filename)?.len();
    println!("Successfully generated '{}'", output_filename);
    println!("SCIP JSON file size: {} bytes", file_size);

    Ok(output_filename)
}

