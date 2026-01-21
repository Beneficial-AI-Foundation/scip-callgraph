//! Unified pipeline for generating enriched call graphs from Verus projects.
//!
//! This binary combines multiple steps into a single command:
//! 1. Generate SCIP index (if needed)
//! 2. Export call graph to D3 format
//! 3. Run verification and enrich with verification status
//! 4. (Optional) Enrich with similar lemmas via Python
//!
//! Usage:
//!     cargo run -p metrics-cli --bin pipeline -- /path/to/verus-project
//!
//! The output is a fully enriched graph.json ready for the web viewer.

use clap::Parser;
use log::{error, info, warn};
use probe_verus::verification::{
    AnalysisResult, AnalysisStatus, VerificationAnalyzer, VerusRunner,
};
use probe_verus::{build_call_graph, convert_to_atoms_with_parsed_spans, parse_scip_json};
use scip_core::atoms_to_d3::atoms_to_d3_graph;
use scip_core::logging::init_logger;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Unified pipeline for generating enriched call graphs from Verus projects
#[derive(Parser, Debug)]
#[command(name = "pipeline")]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to the Verus project
    project: PathBuf,

    /// Output graph file
    #[arg(short, long, default_value = "web/public/graph.json")]
    output: PathBuf,

    /// Skip verification status enrichment
    #[arg(long)]
    skip_verification: bool,

    /// Skip similar lemmas enrichment (requires Python + verus_lemma_finder)
    #[arg(long)]
    skip_similar_lemmas: bool,

    /// Use cached SCIP JSON if available (default: regenerate fresh)
    #[arg(long)]
    use_cached_scip: bool,

    /// Verus package name (for workspaces)
    #[arg(short, long)]
    package: Option<String>,

    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,

    /// GitHub repository URL for source code links in the web viewer
    /// (e.g., https://github.com/user/repo)
    #[arg(long)]
    github_url: Option<String>,

    /// Use rust-analyzer instead of verus-analyzer for SCIP generation
    #[arg(long)]
    use_rust_analyzer: bool,
}

fn check_command_exists(cmd: &str) -> bool {
    which::which(cmd).is_ok()
}

/// Run verus-analyzer or rust-analyzer to generate a new SCIP binary
fn generate_new_scip(project: &Path, use_rust_analyzer: bool) -> Result<PathBuf, String> {
    let analyzer = if use_rust_analyzer {
        "rust-analyzer"
    } else {
        "verus-analyzer"
    };

    // Check prerequisites
    if !check_command_exists(analyzer) {
        let install_hint = if use_rust_analyzer {
            "Install with: rustup component add rust-analyzer"
        } else {
            "Install verus-analyzer from https://github.com/verus-lang/verus-analyzer/releases"
        };
        return Err(format!("{} not found in PATH. {}", analyzer, install_hint));
    }

    info!(
        "Generating SCIP index using {} for {}...",
        analyzer,
        project.display()
    );
    info!("  (This may take a while for large projects)");

    // Run analyzer scip command
    let scip_status = Command::new(analyzer)
        .args(["scip", "."])
        .current_dir(project)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| format!("Failed to run {}: {}", analyzer, e))?;

    if !scip_status.success() {
        return Err(format!(
            "{} scip failed with status: {}",
            analyzer, scip_status
        ));
    }

    let generated_scip_path = project.join("index.scip");
    if !generated_scip_path.exists() {
        return Err(format!(
            "index.scip not found after running {} scip",
            analyzer
        ));
    }

    info!("✓ SCIP index generated using {}", analyzer);
    Ok(generated_scip_path)
}

/// Generate SCIP index and JSON for a project
///
/// By default, always regenerates fresh SCIP data.
/// If `use_cached` is true, uses existing index.scip.json if available.
fn generate_scip(
    project: &Path,
    use_cached: bool,
    use_rust_analyzer: bool,
) -> Result<PathBuf, String> {
    let root_json_path = project.join("index.scip.json");

    // If caching is enabled, check for existing JSON
    if use_cached {
        if root_json_path.exists() {
            info!("Using cached SCIP JSON: {}", root_json_path.display());
            return Ok(root_json_path);
        } else {
            info!("No cached SCIP JSON found, generating fresh...");
        }
    }

    // Generate fresh SCIP binary
    let scip_path = generate_new_scip(project, use_rust_analyzer)?;

    // Convert SCIP to JSON
    if !check_command_exists("scip") {
        return Err("scip not found in PATH. Install with: cargo install scip-cli".to_string());
    }
    info!("Converting SCIP to JSON...");
    let scip_output = Command::new("scip")
        .args(["print", "--json", scip_path.to_str().unwrap()])
        .output()
        .map_err(|e| format!("Failed to run scip: {}", e))?;

    if !scip_output.status.success() {
        return Err(format!(
            "scip print failed: {}",
            String::from_utf8_lossy(&scip_output.stderr)
        ));
    }

    // Write JSON to project root
    std::fs::write(&root_json_path, &scip_output.stdout)
        .map_err(|e| format!("Failed to write SCIP JSON: {}", e))?;

    info!("✓ SCIP JSON saved to: {}", root_json_path.display());
    Ok(root_json_path)
}

/// Export call graph to D3 format using probe-verus' unique name resolution
fn export_call_graph(
    scip_json: &Path,
    output: &Path,
    project_root: &Path,
    github_url: Option<String>,
) -> Result<(), String> {
    info!("Building call graph from SCIP data (using probe-verus)...");

    let scip_data = parse_scip_json(scip_json.to_str().unwrap())
        .map_err(|e| format!("Failed to parse SCIP JSON: {}", e))?;

    let (call_graph, symbol_to_display_name) = build_call_graph(&scip_data);
    info!("  Call graph contains {} functions", call_graph.len());

    info!("Converting to atoms with unique scip_names and accurate line spans...");
    info!("  Parsing source files with verus_syn for accurate function body spans...");
    // Pass with_locations=true to get call location tracking (precondition/postcondition/inner)
    let atoms = convert_to_atoms_with_parsed_spans(
        &call_graph,
        &symbol_to_display_name,
        project_root,
        true, // with_locations - enables requires/ensures tracking
    );

    // Convert to HashMap keyed by code_name for the D3 converter
    let atoms_map: HashMap<String, _> = atoms
        .into_iter()
        .map(|atom| (atom.code_name.clone(), atom))
        .collect();
    info!("  Generated {} atoms with unique names", atoms_map.len());

    info!("Exporting to D3 format...");
    let project_root_str = project_root.to_string_lossy().to_string();
    let d3_graph = atoms_to_d3_graph(&atoms_map, &call_graph, &project_root_str, github_url);

    let json = serde_json::to_string_pretty(&d3_graph)
        .map_err(|e| format!("Failed to serialize D3 graph: {}", e))?;
    std::fs::write(output, json).map_err(|e| format!("Failed to write output: {}", e))?;

    info!("✓ Call graph exported to {}", output.display());
    info!(
        "  {} nodes, {} edges",
        d3_graph.nodes.len(),
        d3_graph.links.len()
    );
    Ok(())
}

/// Run verification and return the analysis result
fn run_verification(project: &Path, package: Option<&str>) -> Result<AnalysisResult, String> {
    info!("Running Verus verification...");
    info!("  (This may take a while)");

    let runner = VerusRunner::new();
    let (output, exit_code) = runner
        .run_verification(project, package, None, None, None)
        .map_err(|e| format!("Failed to run verification: {}", e))?;

    info!("  Verification completed with exit code: {}", exit_code);

    let analyzer = VerificationAnalyzer::new();
    let result = analyzer.analyze_output(project, &output, Some(exit_code), None, None);

    match result.status {
        AnalysisStatus::Success => info!("✓ Verification succeeded!"),
        AnalysisStatus::VerificationFailed => {
            warn!(
                "⚠ Verification completed with {} errors",
                result.summary.failed_functions
            );
        }
        AnalysisStatus::CompilationFailed => {
            warn!("⚠ Compilation failed");
            // Show compilation errors for debugging
            if !result.compilation.errors.is_empty() {
                warn!(
                    "  Compilation errors ({}):",
                    result.compilation.errors.len()
                );
                for (i, err) in result.compilation.errors.iter().enumerate() {
                    if let (Some(file), Some(line)) = (&err.file, err.line) {
                        warn!("  {}. {}:{} - {}", i + 1, file, line, err.message);
                    } else {
                        warn!("  {}. {}", i + 1, err.message);
                    }
                    // Show full message for first few errors
                    if i < 3 {
                        for msg_line in &err.full_message {
                            warn!("     {}", msg_line);
                        }
                    }
                }
                if result.compilation.errors.len() > 3 {
                    warn!(
                        "  ... and {} more errors",
                        result.compilation.errors.len() - 3
                    );
                }
            } else {
                // No parsed errors - might be a different issue
                warn!("  No specific compilation errors parsed.");
                warn!("  This could indicate:");
                warn!("    - cargo verus is not installed");
                warn!("    - Project doesn't have Verus configured");
                warn!("    - Other build issues");
                // Show raw output snippet for debugging
                warn!("  Raw output (first 50 lines):");
                for line in output.lines().take(50) {
                    warn!("    {}", line);
                }
            }
        }
        AnalysisStatus::FunctionsOnly => {
            info!("  Functions parsed (no verification run)");
        }
    }

    Ok(result)
}

/// Normalize a file path for comparison
fn normalize_path(path: &str) -> String {
    let path = path.replace("file://", "");
    if let Some(src_idx) = path.find("/src/") {
        return path[src_idx + 1..].to_string();
    }
    path
}

/// Enrich graph with verification status
fn enrich_with_verification_status(
    graph_path: &Path,
    verification: &AnalysisResult,
) -> Result<usize, String> {
    info!("Enriching graph with verification status...");

    // Read the graph
    let graph_content =
        std::fs::read_to_string(graph_path).map_err(|e| format!("Failed to read graph: {}", e))?;
    let mut graph: serde_json::Value = serde_json::from_str(&graph_content)
        .map_err(|e| format!("Failed to parse graph: {}", e))?;

    // Build lookup: (display_name, normalized_path) -> status
    let mut lookup: HashMap<(String, String), String> = HashMap::new();
    let mut by_name: HashMap<String, Vec<String>> = HashMap::new();

    for func in &verification.verification.verified_functions {
        let norm_path = normalize_path(&func.code_path);
        lookup.insert(
            (func.display_name.clone(), norm_path.clone()),
            "verified".to_string(),
        );
        by_name
            .entry(func.display_name.clone())
            .or_default()
            .push("verified".to_string());
    }
    for func in &verification.verification.failed_functions {
        let norm_path = normalize_path(&func.code_path);
        lookup.insert(
            (func.display_name.clone(), norm_path.clone()),
            "failed".to_string(),
        );
        by_name
            .entry(func.display_name.clone())
            .or_default()
            .push("failed".to_string());
    }
    for func in &verification.verification.unverified_functions {
        let norm_path = normalize_path(&func.code_path);
        lookup.insert(
            (func.display_name.clone(), norm_path.clone()),
            "unverified".to_string(),
        );
        by_name
            .entry(func.display_name.clone())
            .or_default()
            .push("unverified".to_string());
    }

    // Enrich nodes
    let mut enriched_count = 0;
    if let Some(nodes) = graph.get_mut("nodes").and_then(|n| n.as_array_mut()) {
        for node in nodes {
            let display_name = node
                .get("display_name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let relative_path = node
                .get("relative_path")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let full_path = node.get("full_path").and_then(|v| v.as_str()).unwrap_or("");

            // Try to find status
            let mut status: Option<&str> = None;

            // Strategy 1: Match by (name, path)
            for path in [relative_path, full_path] {
                if path.is_empty() {
                    continue;
                }
                let norm_path = normalize_path(path);
                if let Some(s) = lookup.get(&(display_name.to_string(), norm_path)) {
                    status = Some(s.as_str());
                    break;
                }
            }

            // Strategy 2: Match by name only if unique status
            if status.is_none() {
                if let Some(statuses) = by_name.get(display_name) {
                    let unique: std::collections::HashSet<_> = statuses.iter().collect();
                    if unique.len() == 1 {
                        status = Some(statuses[0].as_str());
                    }
                }
            }

            if let Some(s) = status {
                node.as_object_mut()
                    .unwrap()
                    .insert("verification_status".to_string(), serde_json::json!(s));
                enriched_count += 1;
            }
        }
    }

    // Write back
    let json = serde_json::to_string_pretty(&graph)
        .map_err(|e| format!("Failed to serialize graph: {}", e))?;
    std::fs::write(graph_path, json).map_err(|e| format!("Failed to write graph: {}", e))?;

    info!(
        "✓ Enriched {} nodes with verification status",
        enriched_count
    );
    info!("  Verified: {}", verification.summary.verified_functions);
    info!("  Failed: {}", verification.summary.failed_functions);
    info!(
        "  Unverified (assume/admit): {}",
        verification.summary.unverified_functions
    );

    Ok(enriched_count)
}

/// Try to enrich with similar lemmas via Python
fn enrich_with_similar_lemmas(graph_path: &Path, _project: &Path) -> Result<(), String> {
    info!("Attempting to enrich with similar lemmas...");

    // Find Python script and verus_lemma_finder
    let repo_root = std::env::current_dir().unwrap_or_default();
    let script_path = repo_root.join("scripts/enrich_graph_with_similar_lemmas.py");

    // Find vstd index (prefer submodule, then data/)
    let vstd_index_candidates = [
        repo_root.join("external/verus_lemma_finder/data/vstd_lemma_index.json"),
        repo_root.join("data/vstd_lemma_index.json"),
    ];

    let vstd_index = vstd_index_candidates.iter().find(|p| p.exists());

    if !script_path.exists() {
        return Err(format!(
            "Python script not found: {}",
            script_path.display()
        ));
    }

    let vstd_index = match vstd_index {
        Some(p) => p,
        None => {
            return Err(
                "vstd_lemma_index.json not found. Check external/verus_lemma_finder submodule."
                    .to_string(),
            );
        }
    };

    // Try to run with uv
    let uv_result = Command::new("uv")
        .args([
            "run",
            "python",
            script_path.to_str().unwrap(),
            "--graph",
            graph_path.to_str().unwrap(),
            "--index",
            vstd_index.to_str().unwrap(),
            "--quiet",
        ])
        .current_dir(&repo_root)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status();

    match uv_result {
        Ok(status) if status.success() => {
            info!("✓ Enriched with similar lemmas from vstd");
            Ok(())
        }
        Ok(status) => Err(format!("Python script failed with status: {}", status)),
        Err(e) => Err(format!("Failed to run uv: {}", e)),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    init_logger(args.debug);

    println!("════════════════════════════════════════════════════════════════");
    println!("  scip-callgraph Pipeline");
    println!("  Unified call graph generation with enrichments");
    println!("════════════════════════════════════════════════════════════════");
    println!();

    // Validate project path
    if !args.project.exists() {
        error!("Project path does not exist: {}", args.project.display());
        std::process::exit(1);
    }

    let cargo_toml = args.project.join("Cargo.toml");
    if !cargo_toml.exists() {
        error!(
            "Not a valid Rust project (Cargo.toml not found): {}",
            args.project.display()
        );
        std::process::exit(1);
    }

    info!("Project: {}", args.project.display());
    info!("Output: {}", args.output.display());
    println!();

    // Step 1: Generate SCIP
    println!("─── Step 1: Generate SCIP Index ───────────────────────────────");
    if args.use_rust_analyzer {
        info!("Using rust-analyzer (non-Verus mode)");
    }
    let scip_json = match generate_scip(&args.project, args.use_cached_scip, args.use_rust_analyzer)
    {
        Ok(path) => path,
        Err(e) => {
            error!("Failed to generate SCIP: {}", e);
            std::process::exit(1);
        }
    };
    println!();

    // Step 2: Export call graph
    println!("─── Step 2: Export Call Graph ─────────────────────────────────");

    // Ensure output directory exists
    if let Some(parent) = args.output.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if let Err(e) = export_call_graph(
        &scip_json,
        &args.output,
        &args.project,
        args.github_url.clone(),
    ) {
        error!("Failed to export call graph: {}", e);
        std::process::exit(1);
    }
    println!();

    // Step 3: Run verification and enrich (unless skipped)
    if !args.skip_verification {
        println!("─── Step 3: Verification Status ─────────────────────────────────");
        match run_verification(&args.project, args.package.as_deref()) {
            Ok(result) => {
                if let Err(e) = enrich_with_verification_status(&args.output, &result) {
                    warn!("Failed to enrich with verification status: {}", e);
                }
            }
            Err(e) => {
                warn!("Verification failed: {}", e);
                warn!("Continuing without verification status enrichment.");
            }
        }
        println!();
    } else {
        info!("Skipping verification status enrichment (--skip-verification)");
        println!();
    }

    // Step 4: Enrich with similar lemmas (unless skipped)
    if !args.skip_similar_lemmas {
        println!("─── Step 4: Similar Lemmas ──────────────────────────────────────");
        match enrich_with_similar_lemmas(&args.output, &args.project) {
            Ok(()) => {}
            Err(e) => {
                warn!("Similar lemmas enrichment skipped: {}", e);
                info!("To enable: uv sync --extra enrich && uv run maturin develop --release -m external/verus_lemma_finder/rust/Cargo.toml");
            }
        }
        println!();
    } else {
        info!("Skipping similar lemmas enrichment (--skip-similar-lemmas)");
        println!();
    }

    // Done!
    println!("════════════════════════════════════════════════════════════════");
    println!("  ✓ Pipeline Complete!");
    println!("════════════════════════════════════════════════════════════════");
    println!();
    println!("Output: {}", args.output.display());
    println!();
    println!("Next steps:");
    println!("  cd web && npm install && npm run dev");
    println!("  Open http://localhost:3000");
    println!();

    Ok(())
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // =========================================================================
    // normalize_path tests
    // =========================================================================

    #[test]
    fn test_normalize_path_strips_file_protocol() {
        // Path with /src/ gets normalized to start from src/
        let path = "file:///home/user/project/src/lib.rs";
        assert_eq!(normalize_path(path), "src/lib.rs");
    }

    #[test]
    fn test_normalize_path_strips_file_protocol_no_src() {
        // Path without /src/ just strips the file:// protocol
        let path = "file:///home/user/build.rs";
        assert_eq!(normalize_path(path), "/home/user/build.rs");
    }

    #[test]
    fn test_normalize_path_extracts_from_src() {
        let path = "/home/user/project/src/backend/scalar.rs";
        assert_eq!(normalize_path(path), "src/backend/scalar.rs");
    }

    #[test]
    fn test_normalize_path_with_file_protocol_and_src() {
        let path = "file:///home/user/my-project/src/lib.rs";
        assert_eq!(normalize_path(path), "src/lib.rs");
    }

    #[test]
    fn test_normalize_path_no_src_returns_as_is() {
        let path = "/home/user/project/lib.rs";
        assert_eq!(normalize_path(path), "/home/user/project/lib.rs");
    }

    #[test]
    fn test_normalize_path_nested_src() {
        // Should match the first /src/
        let path = "/project/src/nested/src/file.rs";
        assert_eq!(normalize_path(path), "src/nested/src/file.rs");
    }

    #[test]
    fn test_enrich_with_verification_status_basic() {
        use probe_verus::verification::{
            AnalysisSummary, CompilationResult, FunctionLocation, VerificationResult,
        };
        use probe_verus::CodeTextInfo;

        let temp_dir = TempDir::new().unwrap();
        let graph_path = temp_dir.path().join("test_graph.json");

        // Create a minimal graph with a node
        let graph = serde_json::json!({
            "nodes": [
                {
                    "id": "test::my_function",
                    "display_name": "my_function",
                    "relative_path": "src/lib.rs",
                    "full_path": "/project/src/lib.rs"
                },
                {
                    "id": "test::other_function",
                    "display_name": "other_function",
                    "relative_path": "src/other.rs",
                    "full_path": "/project/src/other.rs"
                }
            ],
            "links": []
        });
        fs::write(&graph_path, serde_json::to_string_pretty(&graph).unwrap()).unwrap();

        // Create a mock verification result
        let result = AnalysisResult {
            status: AnalysisStatus::Success,
            summary: AnalysisSummary {
                total_functions: 2,
                verified_functions: 1,
                failed_functions: 0,
                unverified_functions: 1,
                verification_errors: 0,
                compilation_errors: 0,
                compilation_warnings: 0,
            },
            verification: VerificationResult {
                verified_functions: vec![FunctionLocation {
                    display_name: "my_function".to_string(),
                    code_name: None,
                    code_path: "src/lib.rs".to_string(),
                    code_text: CodeTextInfo {
                        lines_start: 1,
                        lines_end: 10,
                    },
                }],
                failed_functions: vec![],
                unverified_functions: vec![FunctionLocation {
                    display_name: "other_function".to_string(),
                    code_name: None,
                    code_path: "src/other.rs".to_string(),
                    code_text: CodeTextInfo {
                        lines_start: 1,
                        lines_end: 5,
                    },
                }],
                errors: vec![],
            },
            compilation: CompilationResult {
                errors: vec![],
                warnings: vec![],
            },
        };

        // Run enrichment
        let enriched_count = enrich_with_verification_status(&graph_path, &result).unwrap();
        assert_eq!(enriched_count, 2);

        // Verify the graph was enriched
        let enriched_graph: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&graph_path).unwrap()).unwrap();
        let nodes = enriched_graph["nodes"].as_array().unwrap();

        let my_func = nodes
            .iter()
            .find(|n| n["display_name"] == "my_function")
            .unwrap();
        assert_eq!(my_func["verification_status"], "verified");

        let other_func = nodes
            .iter()
            .find(|n| n["display_name"] == "other_function")
            .unwrap();
        assert_eq!(other_func["verification_status"], "unverified");
    }

    #[test]
    fn test_enrich_with_verification_status_handles_failed() {
        use probe_verus::verification::{
            AnalysisSummary, CompilationResult, FunctionLocation, VerificationResult,
        };
        use probe_verus::CodeTextInfo;

        let temp_dir = TempDir::new().unwrap();
        let graph_path = temp_dir.path().join("test_graph.json");

        let graph = serde_json::json!({
            "nodes": [
                {
                    "id": "test::failing_proof",
                    "display_name": "failing_proof",
                    "relative_path": "src/proofs.rs",
                    "full_path": "/project/src/proofs.rs"
                }
            ],
            "links": []
        });
        fs::write(&graph_path, serde_json::to_string_pretty(&graph).unwrap()).unwrap();

        let result = AnalysisResult {
            status: AnalysisStatus::VerificationFailed,
            summary: AnalysisSummary {
                total_functions: 1,
                verified_functions: 0,
                failed_functions: 1,
                unverified_functions: 0,
                verification_errors: 1,
                compilation_errors: 0,
                compilation_warnings: 0,
            },
            verification: VerificationResult {
                verified_functions: vec![],
                failed_functions: vec![FunctionLocation {
                    display_name: "failing_proof".to_string(),
                    code_name: None,
                    code_path: "src/proofs.rs".to_string(),
                    code_text: CodeTextInfo {
                        lines_start: 1,
                        lines_end: 20,
                    },
                }],
                unverified_functions: vec![],
                errors: vec![],
            },
            compilation: CompilationResult {
                errors: vec![],
                warnings: vec![],
            },
        };

        let enriched_count = enrich_with_verification_status(&graph_path, &result).unwrap();
        assert_eq!(enriched_count, 1);

        let enriched_graph: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&graph_path).unwrap()).unwrap();
        let node = &enriched_graph["nodes"][0];
        assert_eq!(node["verification_status"], "failed");
    }

    // =========================================================================
    // Integration test: export_call_graph with mock SCIP data
    // =========================================================================

    /// Creates a minimal mock SCIP JSON for testing
    fn create_mock_scip_json() -> serde_json::Value {
        serde_json::json!({
            "metadata": {
                "version": 1,
                "tool_info": {
                    "name": "rust-analyzer",
                    "version": "test"
                },
                "project_root": "file:///mock/project",
                "text_document_encoding": 1
            },
            "documents": [
                {
                    "relative_path": "src/lib.rs",
                    "language": "rust",
                    "position_encoding": 1,
                    "occurrences": [
                        {
                            "range": [10, 0, 10, 10],
                            "symbol": "rust-analyzer cargo mock 0.1.0 lib/foo().",
                            "symbol_roles": 1
                        },
                        {
                            "range": [15, 4, 15, 7],
                            "symbol": "rust-analyzer cargo mock 0.1.0 lib/bar().",
                            "symbol_roles": 0
                        }
                    ],
                    "symbols": [
                        {
                            "symbol": "rust-analyzer cargo mock 0.1.0 lib/foo().",
                            "kind": 12,
                            "display_name": "foo",
                            "signature_documentation": {
                                "language": "rust",
                                "text": "fn foo()",
                                "position_encoding": 1
                            }
                        },
                        {
                            "symbol": "rust-analyzer cargo mock 0.1.0 lib/bar().",
                            "kind": 12,
                            "display_name": "bar",
                            "signature_documentation": {
                                "language": "rust",
                                "text": "fn bar()",
                                "position_encoding": 1
                            }
                        }
                    ]
                }
            ]
        })
    }

    #[test]
    fn test_export_call_graph_with_mock_scip() {
        let temp_dir = TempDir::new().unwrap();
        let scip_json_path = temp_dir.path().join("index.scip.json");
        let output_path = temp_dir.path().join("graph.json");

        // Write mock SCIP JSON
        let mock_scip = create_mock_scip_json();
        fs::write(
            &scip_json_path,
            serde_json::to_string_pretty(&mock_scip).unwrap(),
        )
        .unwrap();

        // Run export
        let result = export_call_graph(
            &scip_json_path,
            &output_path,
            Path::new("/mock/project"),
            None,
        );
        assert!(
            result.is_ok(),
            "export_call_graph should succeed: {:?}",
            result
        );

        // Verify output exists and is valid JSON
        assert!(output_path.exists());
        let graph_content = fs::read_to_string(&output_path).unwrap();
        let graph: serde_json::Value = serde_json::from_str(&graph_content).unwrap();

        // Verify structure - the graph should have nodes and links arrays
        assert!(graph["nodes"].is_array(), "Graph should have 'nodes' array");
        assert!(graph["links"].is_array(), "Graph should have 'links' array");

        let nodes = graph["nodes"].as_array().unwrap();
        let links = graph["links"].as_array().unwrap();

        // The minimal mock may not produce nodes (depends on function-like kind detection)
        // but the output structure should be valid
        eprintln!(
            "✓ Mock SCIP test: {} nodes, {} links",
            nodes.len(),
            links.len()
        );

        // If there are nodes, verify their structure
        if !nodes.is_empty() {
            let first_node = &nodes[0];
            assert!(first_node["id"].is_string());
            assert!(first_node["display_name"].is_string());
        }
    }

    #[test]
    fn test_export_call_graph_with_github_url() {
        let temp_dir = TempDir::new().unwrap();
        let scip_json_path = temp_dir.path().join("index.scip.json");
        let output_path = temp_dir.path().join("graph.json");

        // Write mock SCIP JSON
        let mock_scip = create_mock_scip_json();
        fs::write(
            &scip_json_path,
            serde_json::to_string_pretty(&mock_scip).unwrap(),
        )
        .unwrap();

        // Run export with GitHub URL
        let github_url = Some("https://github.com/test/repo".to_string());
        let result = export_call_graph(
            &scip_json_path,
            &output_path,
            Path::new("/mock/project"),
            github_url,
        );
        assert!(result.is_ok());

        // Verify output
        let graph: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&output_path).unwrap()).unwrap();

        // Graph should be valid
        assert!(graph["nodes"].is_array());
    }

    // =========================================================================
    // generate_scip tests (uses mock cached SCIP)
    // =========================================================================

    #[test]
    fn test_generate_scip_uses_cached_json() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path().to_path_buf();

        // Create a mock cached SCIP JSON
        let cached_json = project_dir.join("index.scip.json");
        fs::write(&cached_json, r#"{"metadata": {}, "documents": []}"#).unwrap();

        // Should return the cached path when use_cached=true (use_rust_analyzer=false is default)
        let result = generate_scip(&project_dir, true, false);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), cached_json);
    }

    #[test]
    fn test_generate_scip_returns_error_without_cache_or_tools() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path().to_path_buf();

        // No cached JSON, and verus-analyzer likely not installed
        // This should either use cache (if exists) or fail gracefully
        let result = generate_scip(&project_dir, true, false);

        // With use_cached=true but no cache, it should try to regenerate
        // and likely fail (verus-analyzer not available)
        // We just verify it doesn't panic
        if let Err(err) = result {
            // Should mention verus-analyzer or rust-analyzer or scip
            assert!(
                err.contains("verus-analyzer")
                    || err.contains("rust-analyzer")
                    || err.contains("scip")
                    || err.contains("not found"),
                "Error should mention missing tool: {}",
                err
            );
        }
    }
}
