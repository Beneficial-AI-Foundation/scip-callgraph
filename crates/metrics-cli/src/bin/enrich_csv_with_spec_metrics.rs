use csv::{Reader, Writer};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;

#[derive(Debug, Deserialize)]
struct AtomWithMetrics {
    #[allow(dead_code)]
    identifier: String,
    #[allow(dead_code)]
    statement_type: String,
    #[allow(dead_code)]
    deps: Vec<String>,
    #[allow(dead_code)]
    body: String,
    display_name: String,
    full_path: String,
    #[allow(dead_code)]
    relative_path: String,
    #[allow(dead_code)]
    file_name: String,
    #[allow(dead_code)]
    parent_folder: String,
    metrics: FunctionMetrics,
}

#[derive(Debug, Deserialize)]
struct FunctionMetrics {
    #[allow(dead_code)]
    requires_count: usize,
    #[allow(dead_code)]
    requires_lengths: Vec<usize>,
    requires_specs: Vec<SpecHalsteadMetrics>,
    #[allow(dead_code)]
    ensures_count: usize,
    #[allow(dead_code)]
    ensures_lengths: Vec<usize>,
    ensures_specs: Vec<SpecHalsteadMetrics>,
    decreases_count: usize,
    #[allow(dead_code)]
    decreases_specs: Vec<SpecHalsteadMetrics>,
    #[allow(dead_code)]
    body_length: usize,
    #[allow(dead_code)]
    operators: HashMap<String, usize>,
}

#[derive(Debug, Deserialize)]
struct SpecHalsteadMetrics {
    #[allow(dead_code)]
    text: String,
    halstead_length: Option<usize>,
    halstead_difficulty: Option<f64>,
    halstead_effort: Option<f64>,
    #[allow(dead_code)]
    halstead_vocabulary: Option<usize>,
    #[allow(dead_code)]
    halstead_volume: Option<f64>,
    #[allow(dead_code)]
    unique_operators: Option<usize>,
    #[allow(dead_code)]
    total_operators: Option<usize>,
    #[allow(dead_code)]
    unique_operands: Option<usize>,
    #[allow(dead_code)]
    total_operands: Option<usize>,
    #[allow(dead_code)]
    parse_error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CsvInputRow {
    function: String,
    module: String,
    cyclomatic: String,
    cognitive: String,
    halstead_difficulty: String,
    halstead_effort: String,
    halstead_length: String,
}

#[derive(Debug, Serialize)]
struct CsvOutputRow {
    function: String,
    module: String,
    cyclomatic: String,
    cognitive: String,
    halstead_difficulty: String,
    halstead_effort: String,
    halstead_length: String,
    requires_halstead_length: String,
    requires_halstead_difficulty: String,
    requires_halstead_effort: String,
    ensures_halstead_length: String,
    ensures_halstead_difficulty: String,
    ensures_halstead_effort: String,
    decreases_count: String,
}

/// Aggregate Halstead metrics from a list of specs
/// Returns (total_length, avg_difficulty, total_effort)
fn aggregate_spec_metrics(
    specs: &[SpecHalsteadMetrics],
) -> (Option<usize>, Option<f64>, Option<f64>) {
    let valid_specs: Vec<&SpecHalsteadMetrics> =
        specs.iter().filter(|s| s.parse_error.is_none()).collect();

    if valid_specs.is_empty() {
        return (None, None, None);
    }

    // Sum lengths
    let total_length: usize = valid_specs.iter().filter_map(|s| s.halstead_length).sum();

    // Average difficulty
    let difficulties: Vec<f64> = valid_specs
        .iter()
        .filter_map(|s| s.halstead_difficulty)
        .collect();
    let avg_difficulty = if !difficulties.is_empty() {
        Some(difficulties.iter().sum::<f64>() / difficulties.len() as f64)
    } else {
        None
    };

    // Sum efforts
    let total_effort: f64 = valid_specs.iter().filter_map(|s| s.halstead_effort).sum();

    (Some(total_length), avg_difficulty, Some(total_effort))
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 {
        eprintln!("Usage: {} <metrics_json> <input_csv> <output_csv>", args[0]);
        eprintln!();
        eprintln!("Example:");
        eprintln!("  {} \\", args[0]);
        eprintln!("    curve_dalek_atoms_with_spec_halstead_v18.json \\");
        eprintln!("    functions_to_track_enriched.csv \\");
        eprintln!("    functions_to_track_enriched_with_specs.csv");
        std::process::exit(1);
    }

    let metrics_json_path = &args[1];
    let input_csv_path = &args[2];
    let output_csv_path = &args[3];

    println!("Loading metrics from {}...", metrics_json_path);
    let file = File::open(metrics_json_path)?;
    let atoms: Vec<AtomWithMetrics> = serde_json::from_reader(file)?;
    println!("  Loaded {} functions with metrics", atoms.len());

    // Build lookup maps by display_name and full_path
    let mut by_display_name: HashMap<String, &AtomWithMetrics> = HashMap::new();
    let mut by_full_path: HashMap<String, &AtomWithMetrics> = HashMap::new();

    for atom in &atoms {
        by_display_name.insert(atom.display_name.clone(), atom);
        by_full_path.insert(atom.full_path.clone(), atom);
    }

    println!("Loading CSV from {}...", input_csv_path);
    let mut reader = Reader::from_path(input_csv_path)?;
    let mut writer = Writer::from_path(output_csv_path)?;

    let mut matched = 0;
    let mut total = 0;

    for result in reader.deserialize() {
        let input_row: CsvInputRow = result?;
        total += 1;

        // Try to find the function in the metrics JSON
        // Try multiple key formats
        let function_name = &input_row.function;
        let module = &input_row.module;

        let mut atom_opt: Option<&AtomWithMetrics> = None;

        // Strategy 1: Try display_name directly
        if let Some(atom) = by_display_name.get(function_name) {
            atom_opt = Some(atom);
        }

        // Strategy 2: Try module::function
        if atom_opt.is_none() {
            let key = format!("{}::{}", module, function_name);
            atom_opt = by_full_path
                .get(&key)
                .copied()
                .or_else(|| by_display_name.get(&key).copied());
        }

        // Strategy 3: Extract last segment of module and try module::Type::function
        if atom_opt.is_none() && module.contains("::") {
            let parts: Vec<&str> = module.split("::").collect();
            if let Some(last_part) = parts.last() {
                let key = format!("{}::{}::{}", module, last_part, function_name);
                atom_opt = by_full_path
                    .get(&key)
                    .copied()
                    .or_else(|| by_display_name.get(&key).copied());
            }
        }

        // Strategy 4: Try just the function name in full_path
        if atom_opt.is_none() {
            for (full_path, atom) in &by_full_path {
                if full_path.ends_with(&format!("::{}", function_name))
                    || full_path == function_name
                {
                    atom_opt = Some(atom);
                    break;
                }
            }
        }

        let output_row = if let Some(atom) = atom_opt {
            matched += 1;

            // Aggregate requires metrics
            let (req_length, req_difficulty, req_effort) =
                aggregate_spec_metrics(&atom.metrics.requires_specs);

            // Aggregate ensures metrics
            let (ens_length, ens_difficulty, ens_effort) =
                aggregate_spec_metrics(&atom.metrics.ensures_specs);

            CsvOutputRow {
                function: input_row.function,
                module: input_row.module,
                cyclomatic: input_row.cyclomatic,
                cognitive: input_row.cognitive,
                halstead_difficulty: input_row.halstead_difficulty,
                halstead_effort: input_row.halstead_effort,
                halstead_length: input_row.halstead_length,
                requires_halstead_length: req_length.map_or(String::new(), |v| v.to_string()),
                requires_halstead_difficulty: req_difficulty
                    .map_or(String::new(), |v| format!("{:.2}", v)),
                requires_halstead_effort: req_effort.map_or(String::new(), |v| format!("{:.2}", v)),
                ensures_halstead_length: ens_length.map_or(String::new(), |v| v.to_string()),
                ensures_halstead_difficulty: ens_difficulty
                    .map_or(String::new(), |v| format!("{:.2}", v)),
                ensures_halstead_effort: ens_effort.map_or(String::new(), |v| format!("{:.2}", v)),
                decreases_count: if atom.metrics.decreases_count > 0 {
                    atom.metrics.decreases_count.to_string()
                } else {
                    String::new()
                },
            }
        } else {
            // No match found - keep original data, add empty spec metrics
            CsvOutputRow {
                function: input_row.function,
                module: input_row.module,
                cyclomatic: input_row.cyclomatic,
                cognitive: input_row.cognitive,
                halstead_difficulty: input_row.halstead_difficulty,
                halstead_effort: input_row.halstead_effort,
                halstead_length: input_row.halstead_length,
                requires_halstead_length: String::new(),
                requires_halstead_difficulty: String::new(),
                requires_halstead_effort: String::new(),
                ensures_halstead_length: String::new(),
                ensures_halstead_difficulty: String::new(),
                ensures_halstead_effort: String::new(),
                decreases_count: String::new(),
            }
        };

        writer.serialize(output_row)?;
    }

    writer.flush()?;

    println!();
    println!("✓ Done!");
    println!();
    println!("Results:");
    println!("  Total rows: {}", total);
    println!(
        "  Matched: {} ({:.1}%)",
        matched,
        (matched as f64 / total as f64) * 100.0
    );
    println!("  Unmatched: {}", total - matched);
    println!();
    println!("Output written to: {}", output_csv_path);

    Ok(())
}
