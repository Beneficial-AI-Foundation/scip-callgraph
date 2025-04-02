use std::path::PathBuf;
use std::fs;

/// Example on how to use the library to build rustdoc JSON. Run it like this:
/// ```bash
/// cargo run --example build-rustdoc-json path/to/Cargo.toml
/// ```
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get manifest path from args or use default
    let manifest_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("Cargo.toml"));

    if !manifest_path.exists() {
        return Err(format!(
            "Cargo.toml not found at {:?}. Usage: cargo run --bin build_rustdoc_json [path/to/Cargo.toml]",
            manifest_path
        ).into());
    }

    println!("Building rustdoc JSON for {:?}", manifest_path);

    // Build it
    let json_path = rustdoc_json::Builder::default()
        .toolchain("nightly")
        .manifest_path(manifest_path)
        .build()?;
    
    println!("Built rustdoc JSON at {:?}", &json_path);

    // Read the JSON file
    let json_content = fs::read_to_string(&json_path)?;
    
    // Parse and pretty print the JSON
    let json_value: serde_json::Value = serde_json::from_str(&json_content)?;
    let pretty_json = serde_json::to_string_pretty(&json_value)?;
    
    // Write to a new file
    let output_path = "rustdoc_output.json";
    fs::write(output_path, pretty_json)?;
    println!("Wrote formatted JSON to {}", output_path);

    Ok(())
}