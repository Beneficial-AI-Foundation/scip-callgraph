use clap::Parser;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

/// Count paths and analyze SCIP index files
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to SCIP index JSON file
    scip_index_file: String,

    /// Enable verbose output showing all paths and filenames
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Deserialize)]
struct ScipIndex {
    documents: Vec<Document>,
}

#[derive(Deserialize)]
struct Document {
    relative_path: String,
}

/// Find all filenames that appear in multiple paths
fn find_duplicate_filenames(documents: &[Document]) -> HashMap<String, Vec<String>> {
    let mut filename_to_paths: HashMap<String, Vec<String>> = HashMap::new();

    for doc in documents {
        if let Some(filename) = Path::new(&doc.relative_path).file_name() {
            if let Some(filename_str) = filename.to_str() {
                filename_to_paths
                    .entry(filename_str.to_owned())
                    .or_default()
                    .push(doc.relative_path.clone());
            }
        }
    }

    // Keep only filenames that appear in multiple paths
    filename_to_paths.retain(|_, paths| paths.len() > 1);
    filename_to_paths
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    println!("Reading SCIP index from {}", args.scip_index_file);
    let contents = fs::read_to_string(&args.scip_index_file)?;
    let index: ScipIndex = serde_json::from_str(&contents)?;

    // Extract all relative paths into a set
    let mut paths = HashSet::new();
    for doc in &index.documents {
        paths.insert(doc.relative_path.clone());
    }

    // Extract filenames from paths
    let mut filenames = HashSet::new();
    for path in &paths {
        if let Some(filename) = Path::new(path).file_name() {
            if let Some(filename_str) = filename.to_str() {
                filenames.insert(filename_str.to_owned());
            }
        }
    }

    println!("Number of unique relative paths: {}", paths.len());
    println!("Number of unique filenames: {}", filenames.len());

    // Find and display paths with duplicate filenames
    let duplicates = find_duplicate_filenames(&index.documents);
    println!(
        "\nNumber of filenames that appear in multiple paths: {}",
        duplicates.len()
    );

    if !duplicates.is_empty() {
        println!("\nFilenames that appear in multiple paths:");
        for (filename, paths) in &duplicates {
            println!("  {} (appears in {} paths):", filename, paths.len());
            for path in paths {
                println!("    {path}");
            }
        }
    }

    // Optional: print all unique paths and filenames for verification
    if args.verbose {
        println!("\nAll unique relative paths:");
        for path in &paths {
            println!("  {path}");
        }

        println!("\nAll unique filenames:");
        for name in &filenames {
            println!("  {name}");
        }
    }

    Ok(())
}
