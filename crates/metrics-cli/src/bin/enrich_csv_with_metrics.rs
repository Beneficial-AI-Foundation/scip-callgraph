/// Enrich functions_to_track.csv with complexity metrics from rust-code-analysis JSONs
///
/// Usage:
///   cargo run --bin enrich_csv_with_metrics \
///     functions_to_track.csv \
///     curve25519-dalek/ \
///     output.csv
use serde::Deserialize;
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct RcaFile {
    name: String,
    #[serde(default)]
    spaces: Vec<RcaSpace>,
}

#[derive(Debug, Deserialize)]
struct RcaSpace {
    name: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    #[allow(dead_code)]
    start_line: Option<usize>,
    #[serde(default)]
    #[allow(dead_code)]
    end_line: Option<usize>,
    #[serde(default)]
    metrics: Option<RcaMetrics>,
    #[serde(default)]
    spaces: Vec<RcaSpace>,
}

#[derive(Debug, Deserialize, Clone)]
struct RcaMetrics {
    #[serde(default)]
    cyclomatic: Option<RcaComplexity>,
    #[serde(default)]
    cognitive: Option<RcaComplexity>,
    #[serde(default)]
    halstead: Option<HalsteadMetrics>,
}

#[derive(Debug, Deserialize, Clone)]
struct RcaComplexity {
    #[serde(default)]
    sum: Option<f64>,
    #[serde(default)]
    #[allow(dead_code)]
    average: Option<f64>,
}

#[derive(Debug, Deserialize, Clone)]
struct HalsteadMetrics {
    #[serde(default)]
    length: Option<f64>,
    #[serde(default)]
    difficulty: Option<f64>,
    #[serde(default)]
    effort: Option<f64>,
}

#[derive(Debug, Default, Clone)]
struct FunctionMetrics {
    cyclomatic: Option<f64>,
    cognitive: Option<f64>,
    halstead_length: Option<f64>,
    halstead_difficulty: Option<f64>,
    halstead_effort: Option<f64>,
}

#[derive(Debug)]
struct CsvFunction {
    function: String,
    module: String,
}

fn collect_functions_recursive(
    space: &RcaSpace,
    file_path: &str,
    parent_name: Option<&str>,
    functions: &mut HashMap<String, FunctionMetrics>,
) {
    if space.kind == "function" {
        if let Some(metrics) = &space.metrics {
            let function_metrics = FunctionMetrics {
                cyclomatic: metrics.cyclomatic.as_ref().and_then(|c| c.sum),
                cognitive: metrics.cognitive.as_ref().and_then(|c| c.sum),
                halstead_length: metrics.halstead.as_ref().and_then(|h| h.length),
                halstead_difficulty: metrics.halstead.as_ref().and_then(|h| h.difficulty),
                halstead_effort: metrics.halstead.as_ref().and_then(|h| h.effort),
            };
            
            // Create keys with and without parent
            if let Some(parent) = parent_name {
                // For methods: Type::method
                let key = format!("{}::{}::{}", file_path, parent, space.name);
                functions.insert(key, function_metrics.clone());
            }
            
            // Always add standalone key as fallback
            let key = format!("{}::{}", file_path, space.name);
            functions.insert(key, function_metrics);
        }
    }
    
    // If this is an impl block, pass its name as parent for nested functions
    let next_parent = if space.kind == "impl" {
        // Clean up impl name (e.g., "&'a FieldElement51" -> "FieldElement51")
        let clean_name = space.name
            .trim_start_matches('&')
            .trim_start_matches("'a ")
            .trim_start_matches("'b ")
            .trim();
        Some(clean_name)
    } else {
        parent_name
    };
    
    for nested in &space.spaces {
        collect_functions_recursive(nested, file_path, next_parent, functions);
    }
}

fn extract_rca_path(rca_file_name: &str) -> Option<String> {
    // Extract path from name like "curve25519-dalek/src/field.rs" or similar
    if let Some(src_idx) = rca_file_name.find("src/") {
        let path = &rca_file_name[src_idx + 4..]; // Skip "src/"
        if let Some(rs_idx) = path.rfind(".rs") {
            return Some(path[..rs_idx].replace('/', "::"));
        }
    }
    None
}

fn load_rca_metrics(rca_output_dir: &Path) -> Result<HashMap<String, FunctionMetrics>, Box<dyn Error>> {
    let mut all_functions: HashMap<String, FunctionMetrics> = HashMap::new();
    let pattern = format!("{}/**/*.json", rca_output_dir.display());
    
    println!("Searching for RCA JSON files in: {}", rca_output_dir.display());
    
    for entry in glob::glob(&pattern)? {
        let json_path = entry?;
        
        let content = match fs::read_to_string(&json_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("  ⚠ Failed to read {}: {}", json_path.display(), e);
                continue;
            }
        };
        
        let rca_file: RcaFile = match serde_json::from_str(&content) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("  ⚠ Failed to parse {}: {}", json_path.display(), e);
                continue;
            }
        };
        
        if let Some(file_path) = extract_rca_path(&rca_file.name) {
            for space in &rca_file.spaces {
                collect_functions_recursive(space, &file_path, None, &mut all_functions);
            }
        }
    }
    
    println!("Loaded metrics for {} functions from RCA JSONs", all_functions.len());
    Ok(all_functions)
}

fn show_sample_keys(rca_metrics: &HashMap<String, FunctionMetrics>, count: usize) {
    println!("\nSample RCA keys (showing first {}):", count);
    for (i, key) in rca_metrics.keys().enumerate() {
        if i >= count {
            break;
        }
        println!("  {}", key);
    }
    println!();
}

fn module_to_file_path(module: &str) -> String {
    // Convert module like "curve25519_dalek::field" to "field"
    // or "curve25519_dalek::backend::serial::u64::field" to "backend::serial::u64::field"
    
    let parts: Vec<&str> = module.split("::").collect();
    if parts.len() > 1 && parts[0] == "curve25519_dalek" {
        parts[1..].join("::")
    } else {
        module.to_string()
    }
}

fn find_function_metrics(
    function_name: &str,
    module: &str,
    rca_metrics: &HashMap<String, FunctionMetrics>,
    debug: bool,
) -> FunctionMetrics {
    let file_path = module_to_file_path(module);
    
    // Try different key patterns
    let mut possible_keys = vec![
        format!("{}::{}", file_path, function_name),
        format!("{}::{}", module, function_name),
    ];
    
    // Add variants with ::mod:: for functions in mod.rs files
    // For "backend::get_selected_backend", try "backend::mod::get_selected_backend"
    // For "backend::serial::curve_models::ProjectivePoint::identity", 
    // try "backend::serial::curve_models::mod::ProjectivePoint::identity"
    
    // Insert ::mod:: after the last path component before the function/type name
    let parts: Vec<&str> = file_path.split("::").collect();
    if !parts.is_empty() {
        // Try adding ::mod:: at the end of the path
        let with_mod = format!("{}::mod::{}", file_path, function_name);
        possible_keys.push(with_mod);
    }
    
    for key in &possible_keys {
        if let Some(metrics) = rca_metrics.get(key) {
            if debug {
                println!("  ✓ Matched '{}' with key: {}", function_name, key);
            }
            return FunctionMetrics {
                cyclomatic: metrics.cyclomatic,
                cognitive: metrics.cognitive,
                halstead_length: metrics.halstead_length,
                halstead_difficulty: metrics.halstead_difficulty,
                halstead_effort: metrics.halstead_effort,
            };
        }
    }
    
    if debug {
        println!("  ✗ No match for '{}' (module: {})", function_name, module);
        println!("    Tried keys:");
        for key in &possible_keys {
            println!("      - {}", key);
        }
        
        // Show similar keys that might exist
        let func_part = function_name.split("::").last().unwrap_or(function_name);
        let similar: Vec<&String> = rca_metrics.keys()
            .filter(|k| k.contains(func_part))
            .take(3)
            .collect();
        
        if !similar.is_empty() {
            println!("    Similar keys in RCA data:");
            for key in similar {
                println!("      - {}", key);
            }
        }
    }
    
    FunctionMetrics::default()
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("Usage: {} <input_csv> <rca_json_dir> <output_csv> [--debug]", args[0]);
        eprintln!("\nExample:");
        eprintln!("  {} functions_to_track.csv curve25519-dalek/ output.csv", args[0]);
        eprintln!("  {} functions_to_track.csv curve25519-dalek/ output.csv --debug", args[0]);
        std::process::exit(1);
    }
    
    let input_csv_path = &args[1];
    let rca_dir = Path::new(&args[2]);
    let output_csv_path = &args[3];
    let debug = args.len() > 4 && args[4] == "--debug";
    
    println!("📊 Enriching CSV with RCA metrics\n");
    
    // Load RCA metrics
    let rca_metrics = load_rca_metrics(rca_dir)?;
    
    if debug {
        show_sample_keys(&rca_metrics, 20);
    }
    
    // Read input CSV
    let mut csv_reader = csv::Reader::from_path(input_csv_path)?;
    let mut functions: Vec<CsvFunction> = Vec::new();
    
    for result in csv_reader.records() {
        let record = result?;
        if record.len() >= 2 {
            functions.push(CsvFunction {
                function: record[0].to_string(),
                module: record[1].to_string(),
            });
        }
    }
    
    println!("Read {} functions from CSV", functions.len());
    
    // Create output CSV
    let mut csv_writer = csv::Writer::from_path(output_csv_path)?;
    
    // Write header
    csv_writer.write_record([
        "function",
        "module",
        "cyclomatic",
        "cognitive",
        "halstead_difficulty",
        "halstead_effort",
        "halstead_length",
    ])?;
    
    let mut matched = 0;
    let mut unmatched = 0;
    
    if debug {
        println!("🔍 Processing {} CSV functions:\n", functions.len());
    }
    
    // Process each function
    for csv_func in &functions {
        let metrics = find_function_metrics(&csv_func.function, &csv_func.module, &rca_metrics, debug);
        
        let has_metrics = metrics.cyclomatic.is_some()
            || metrics.cognitive.is_some()
            || metrics.halstead_length.is_some();
        
        if has_metrics {
            matched += 1;
        } else {
            unmatched += 1;
        }
        
        csv_writer.write_record([
            csv_func.function.clone(),
            csv_func.module.clone(),
            metrics.cyclomatic.map(|v| v.to_string()).unwrap_or_default(),
            metrics.cognitive.map(|v| v.to_string()).unwrap_or_default(),
            metrics.halstead_difficulty.map(|v| v.to_string()).unwrap_or_default(),
            metrics.halstead_effort.map(|v| v.to_string()).unwrap_or_default(),
            metrics.halstead_length.map(|v| v.to_string()).unwrap_or_default(),
        ])?;
    }
    
    csv_writer.flush()?;
    
    println!("\n✅ Results:");
    println!("  Matched: {}/{} ({:.1}%)", matched, functions.len(), (matched as f64 / functions.len() as f64) * 100.0);
    println!("  Unmatched: {}", unmatched);
    println!("\n📝 Output written to: {}", output_csv_path);
    
    Ok(())
}

