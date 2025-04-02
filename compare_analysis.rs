// Using rustdoc-json
fn analyze_with_rustdoc() -> Result<(), Box<dyn std::error::Error>> {
    // Only shows public API relationships
    let json_path = rustdoc_json::Builder::default()
        .toolchain("nightly")
        .manifest_path("Cargo.toml")
        .build()?;
    
    // Can see:
    // - Public function signatures
    // - Documentation
    // - Module structure
    // - Public dependencies
    
    Ok(())
}

// Using rust-analyzer
fn analyze_with_rust_analyzer() -> Result<(), Box<dyn std::error::Error>> {
    // Shows all code relationships
    let analysis = Analysis::new(/* config */)?;
    
    // Can see:
    // - All function calls (public and private)
    // - Implementation details
    // - Runtime call patterns
    // - Actual control flow
    
    Ok(())
}
