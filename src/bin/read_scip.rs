use std::env;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get command line arguments
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        eprintln!("Usage: {} <path_to_index.scip>", args[0]);
        std::process::exit(1);
    }
    
    let scip_path = &args[1];
    
    // Check if file exists
    if !Path::new(scip_path).exists() {
        eprintln!("Error: File not found: {}", scip_path);
        std::process::exit(1);
    }
    
    // Use our SCIP reader to print a summary of the file
    rust_analyzer_test::scip_reader::print_scip_file_summary(scip_path)?;
    
    // Try to extract potential symbol information
    let potential_symbols = rust_analyzer_test::scip_reader::extract_potential_symbols(scip_path)?;
    
    println!("\nIdentified potential symbols in the SCIP file:");
    let display_limit = 20;
    for (i, symbol) in potential_symbols.iter().take(display_limit).enumerate() {
        println!("  {}: {}", i+1, symbol);
    }
    
    if potential_symbols.len() > display_limit {
        println!("  ... and {} more potential symbols", potential_symbols.len() - display_limit);
    }
    
    println!("\nNote: This is a basic extraction based on string patterns. For accurate results,");
    println!("you would need to implement a proper Protocol Buffers parser for the SCIP format.");
    
    Ok(())
}