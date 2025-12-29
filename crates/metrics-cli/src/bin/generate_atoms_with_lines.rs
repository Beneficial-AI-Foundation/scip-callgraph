use scip_core::{build_call_graph, parse_scip_json, FunctionNode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Debug, Serialize, Deserialize)]
struct AtomWithLines {
    #[serde(rename = "display-name")]
    display_name: String,
    visible: bool,
    dependencies: HashMap<String, DependencyInfo>,
    #[serde(rename = "code-path")]
    code_path: String,
    #[serde(rename = "code-function")]
    code_function: String,
    #[serde(rename = "code-text")]
    code_text: CodeTextInfo,
}

#[derive(Debug, Serialize, Deserialize)]
struct DependencyInfo {
    visible: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct CodeTextInfo {
    #[serde(rename = "lines-start")]
    lines_start: usize,
    #[serde(rename = "lines-end")]
    lines_end: usize,
}

fn check_command_exists(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn symbol_to_path(symbol: &str, display_name: &str) -> String {
    scip_core::symbol_to_path(symbol, display_name)
}

fn convert_to_atoms_with_lines(call_graph: &HashMap<String, FunctionNode>) -> Vec<AtomWithLines> {
    call_graph
        .values()
        .map(|node| {
            let mut dependencies = HashMap::new();
            for callee in &node.callees {
                if let Some(callee_node) = call_graph.get(callee) {
                    let dep_path = symbol_to_path(&callee_node.symbol, &callee_node.display_name);
                    dependencies.insert(dep_path, DependencyInfo { visible: true });
                }
            }

            // Calculate lines-start and lines-end from the range
            // SCIP ranges are [start_line, start_col, end_line, end_col]
            let (lines_start, lines_end) = if node.range.len() >= 3 {
                // SCIP line numbers are 0-based, but we'll use them as-is or add 1 if needed
                let start = node.range[0] as usize + 1; // Convert to 1-based
                let end = node.range[2] as usize + 1; // Convert to 1-based
                (start, end)
            } else {
                (0, 0)
            };

            AtomWithLines {
                display_name: node.display_name.clone(),
                visible: true,
                dependencies,
                code_path: node.relative_path.clone(),
                code_function: symbol_to_path(&node.symbol, &node.display_name),
                code_text: CodeTextInfo {
                    lines_start,
                    lines_end,
                },
            }
        })
        .collect()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} <project_path> <output_json>", args[0]);
        eprintln!();
        eprintln!("This tool will:");
        eprintln!("  1. Run 'rust-analyzer scip' on the project");
        eprintln!("  2. Convert index.scip to JSON format");
        eprintln!("  3. Generate an atoms JSON with line numbers instead of function bodies");
        eprintln!();
        eprintln!("Example:");
        eprintln!(
            "  {} ./curve25519-dalek curve_dalek_atoms_with_lines.json",
            args[0]
        );
        eprintln!();
        eprintln!("Prerequisites:");
        eprintln!("  - rust-analyzer must be installed (rustup component add rust-analyzer)");
        eprintln!("  - scip-cli must be installed (cargo install scip-cli)");
        std::process::exit(1);
    }

    let project_path = &args[1];
    let output_path = &args[2];

    println!("═══════════════════════════════════════════════════════════");
    println!("  Generate Atoms JSON with Line Numbers");
    println!("═══════════════════════════════════════════════════════════");
    println!();

    // Check prerequisites
    println!("Checking prerequisites...");

    if !check_command_exists("rust-analyzer") {
        eprintln!("✗ Error: rust-analyzer not found in PATH");
        eprintln!("  Install with: rustup component add rust-analyzer");
        std::process::exit(1);
    }
    println!("  ✓ rust-analyzer found");

    if !check_command_exists("scip") {
        eprintln!("✗ Error: scip not found in PATH");
        eprintln!("  Install with: cargo install scip-cli");
        std::process::exit(1);
    }
    println!("  ✓ scip found");

    // Verify project path exists
    let project_path_buf = PathBuf::from(project_path);
    if !project_path_buf.exists() {
        eprintln!("✗ Error: Project path does not exist: {}", project_path);
        std::process::exit(1);
    }

    // Check if it's a valid Rust project
    let cargo_toml = project_path_buf.join("Cargo.toml");
    if !cargo_toml.exists() {
        eprintln!(
            "✗ Error: Not a valid Rust project (Cargo.toml not found): {}",
            project_path
        );
        std::process::exit(1);
    }
    println!("  ✓ Valid Rust project found");
    println!();

    // Step 1: Run rust-analyzer scip
    println!(
        "Step 1/4: Running rust-analyzer scip on {}...",
        project_path
    );
    println!("  (This may take a while for large projects)");

    // rust-analyzer scip creates index.scip in the current directory
    // We need to cd into the project directory first
    let scip_status = Command::new("rust-analyzer")
        .args(["scip", "."])
        .current_dir(&project_path_buf)
        .status();

    match scip_status {
        Ok(status) if status.success() => {
            println!("  ✓ SCIP index generated successfully");
        }
        Ok(status) => {
            eprintln!("✗ Error: rust-analyzer scip failed with status: {}", status);
            eprintln!("  Make sure the project compiles successfully first");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("✗ Error: Failed to run rust-analyzer: {}", e);
            std::process::exit(1);
        }
    }

    // Path to the generated index.scip file (created in project directory)
    let index_scip_path = project_path_buf.join("index.scip");
    if !index_scip_path.exists() {
        eprintln!(
            "✗ Error: index.scip not found at {}",
            index_scip_path.display()
        );
        eprintln!("  rust-analyzer scip may have failed silently");
        std::process::exit(1);
    }
    println!("  ✓ index.scip created at {}", index_scip_path.display());
    println!();

    // Step 2: Convert SCIP to JSON
    println!("Step 2/4: Converting index.scip to JSON...");

    let temp_json_path = project_path_buf.join("index.scip.json");

    let scip_output = Command::new("scip")
        .args(["print", "--json", index_scip_path.to_str().unwrap()])
        .output();

    match scip_output {
        Ok(output) if output.status.success() => {
            std::fs::write(&temp_json_path, output.stdout).expect("Failed to write SCIP JSON file");
            println!("  ✓ SCIP JSON generated at {}", temp_json_path.display());
        }
        Ok(output) => {
            eprintln!("✗ Error: scip print failed with status: {}", output.status);
            if !output.stderr.is_empty() {
                eprintln!("  stderr: {}", String::from_utf8_lossy(&output.stderr));
            }
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("✗ Error: Failed to run scip: {}", e);
            std::process::exit(1);
        }
    }
    println!();

    // Step 3: Parse SCIP JSON and build call graph
    println!("Step 3/4: Parsing SCIP JSON and building call graph...");

    let scip_index = match parse_scip_json(temp_json_path.to_str().unwrap()) {
        Ok(idx) => idx,
        Err(e) => {
            eprintln!("✗ Failed to parse SCIP JSON: {}", e);
            std::process::exit(1);
        }
    };

    let call_graph = build_call_graph(&scip_index);
    println!("  ✓ Call graph built with {} functions", call_graph.len());
    println!();

    // Step 4: Convert to atoms format with line numbers
    println!("Step 4/4: Converting to atoms format with line numbers...");

    let atoms = convert_to_atoms_with_lines(&call_graph);
    println!("  ✓ Converted {} functions to atoms format", atoms.len());

    // Write the output
    let json = serde_json::to_string_pretty(&atoms).expect("Failed to serialize JSON");
    std::fs::write(output_path, &json).expect("Failed to write output file");

    println!();
    println!("═══════════════════════════════════════════════════════════");
    println!("  ✓ SUCCESS");
    println!("═══════════════════════════════════════════════════════════");
    println!();
    println!("Output written to: {}", output_path);
    println!();
    println!("Summary:");
    println!("  - Total functions: {}", atoms.len());
    println!(
        "  - Total dependencies: {}",
        atoms.iter().map(|a| a.dependencies.len()).sum::<usize>()
    );
    println!("  - Output format: atoms with line numbers and visibility flags");
    println!();

    // Clean up temporary JSON file
    if temp_json_path.exists() {
        let _ = std::fs::remove_file(&temp_json_path);
        println!("Cleaned up temporary file: {}", temp_json_path.display());
    }
}
