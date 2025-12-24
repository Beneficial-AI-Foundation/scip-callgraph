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
use scip_atoms::verification::{AnalysisResult, AnalysisStatus, VerificationAnalyzer, VerusRunner};
use scip_core::logging::init_logger;
use scip_core::scip_to_call_graph_json::{build_call_graph, parse_scip_json};
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
}


fn check_command_exists(cmd: &str) -> bool {
    which::which(cmd).is_ok()
}

/// Run verus-analyzer scip to generate a new SCIP binary
fn generate_new_scip(project: &Path) -> Result<PathBuf, String> {
    // Check prerequisites
    if !check_command_exists("verus-analyzer") {
        return Err("verus-analyzer not found in PATH. Install with: rustup component add verus-analyzer".to_string());
    }

    info!("Generating SCIP index for {}...", project.display());
    info!("  (This may take a while for large projects)");

    // Run verus-analyzer scip
    let scip_status = Command::new("verus-analyzer")
        .args(["scip", "."])
        .current_dir(project)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| format!("Failed to run verus-analyzer: {}", e))?;

    if !scip_status.success() {
        return Err(format!(
            "verus-analyzer scip failed with status: {}",
            scip_status
        ));
    }

    let generated_scip_path = project.join("index.scip");
    if !generated_scip_path.exists() {
        return Err("index.scip not found after running verus-analyzer scip".to_string());
    }

    info!("✓ SCIP index generated");
    Ok(generated_scip_path)
}

/// Generate SCIP index and JSON for a project
/// 
/// By default, always regenerates fresh SCIP data.
/// If `use_cached` is true, uses existing index.scip.json if available.
fn generate_scip(project: &Path, use_cached: bool) -> Result<PathBuf, String> {
    let root_json_path = project.join("index.scip.json");

    // If caching is enabled, check for existing JSON
    if use_cached {
        if root_json_path.exists() {
            info!(
                "Using cached SCIP JSON: {}",
                root_json_path.display()
            );
            return Ok(root_json_path);
        } else {
            info!("No cached SCIP JSON found, generating fresh...");
        }
    }

    // Generate fresh SCIP binary
    let scip_path = generate_new_scip(project)?;

    // Convert SCIP to JSON
    if !check_command_exists("scip") {
        return Err(
            "scip not found in PATH. Install with: cargo install scip-cli".to_string(),
        );
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

/// Export call graph to D3 format
fn export_call_graph(scip_json: &Path, output: &Path) -> Result<(), String> {
    info!("Building call graph from SCIP data...");

    let scip_data = parse_scip_json(scip_json.to_str().unwrap())
        .map_err(|e| format!("Failed to parse SCIP JSON: {}", e))?;

    let call_graph = build_call_graph(&scip_data);
    info!("  Call graph contains {} functions", call_graph.len());

    info!("Exporting to D3 format...");
    scip_core::scip_to_call_graph_json::export_call_graph_d3(&call_graph, &scip_data, output)
        .map_err(|e| format!("Failed to export call graph: {}", e))?;

    info!("✓ Call graph exported to {}", output.display());
    Ok(())
}

/// Set the GitHub URL in the graph metadata
fn set_github_url(graph_path: &Path, github_url: &str) -> Result<(), String> {
    info!("Setting GitHub URL in graph metadata...");
    
    let graph_content = std::fs::read_to_string(graph_path)
        .map_err(|e| format!("Failed to read graph: {}", e))?;
    let mut graph: serde_json::Value =
        serde_json::from_str(&graph_content).map_err(|e| format!("Failed to parse graph: {}", e))?;
    
    // Set the github_url in metadata
    if let Some(metadata) = graph.get_mut("metadata").and_then(|m| m.as_object_mut()) {
        metadata.insert("github_url".to_string(), serde_json::json!(github_url));
    }
    
    // Write back
    let json = serde_json::to_string_pretty(&graph)
        .map_err(|e| format!("Failed to serialize graph: {}", e))?;
    std::fs::write(graph_path, json).map_err(|e| format!("Failed to write graph: {}", e))?;
    
    info!("✓ GitHub URL set to: {}", github_url);
    Ok(())
}

/// Run verification and return the analysis result
fn run_verification(
    project: &Path,
    package: Option<&str>,
) -> Result<AnalysisResult, String> {
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
            warn!("⚠ Verification completed with {} errors", result.summary.failed_functions);
        }
        AnalysisStatus::CompilationFailed => {
            warn!("⚠ Compilation failed");
            // Show compilation errors for debugging
            if !result.compilation.errors.is_empty() {
                warn!("  Compilation errors ({}):", result.compilation.errors.len());
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
                    warn!("  ... and {} more errors", result.compilation.errors.len() - 3);
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
    let graph_content = std::fs::read_to_string(graph_path)
        .map_err(|e| format!("Failed to read graph: {}", e))?;
    let mut graph: serde_json::Value =
        serde_json::from_str(&graph_content).map_err(|e| format!("Failed to parse graph: {}", e))?;

    // Build lookup: (display_name, normalized_path) -> status
    let mut lookup: HashMap<(String, String), String> = HashMap::new();
    let mut by_name: HashMap<String, Vec<String>> = HashMap::new();

    for func in &verification.verification.verified_functions {
        let norm_path = normalize_path(&func.code_path);
        lookup.insert((func.display_name.clone(), norm_path.clone()), "verified".to_string());
        by_name.entry(func.display_name.clone()).or_default().push("verified".to_string());
    }
    for func in &verification.verification.failed_functions {
        let norm_path = normalize_path(&func.code_path);
        lookup.insert((func.display_name.clone(), norm_path.clone()), "failed".to_string());
        by_name.entry(func.display_name.clone()).or_default().push("failed".to_string());
    }
    for func in &verification.verification.unverified_functions {
        let norm_path = normalize_path(&func.code_path);
        lookup.insert((func.display_name.clone(), norm_path.clone()), "unverified".to_string());
        by_name.entry(func.display_name.clone()).or_default().push("unverified".to_string());
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
            let full_path = node
                .get("full_path")
                .and_then(|v| v.as_str())
                .unwrap_or("");

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
    info!("  Unverified (assume/admit): {}", verification.summary.unverified_functions);

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
    
    let vstd_index = vstd_index_candidates
        .iter()
        .find(|p| p.exists());

    if !script_path.exists() {
        return Err(format!(
            "Python script not found: {}",
            script_path.display()
        ));
    }

    let vstd_index = match vstd_index {
        Some(p) => p,
        None => {
            return Err("vstd_lemma_index.json not found. Check external/verus_lemma_finder submodule.".to_string());
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
        Ok(status) => Err(format!(
            "Python script failed with status: {}",
            status
        )),
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
    let scip_json = match generate_scip(&args.project, args.use_cached_scip) {
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
    
    if let Err(e) = export_call_graph(&scip_json, &args.output) {
        error!("Failed to export call graph: {}", e);
        std::process::exit(1);
    }
    
    // Set GitHub URL if provided
    if let Some(ref github_url) = args.github_url {
        if let Err(e) = set_github_url(&args.output, github_url) {
            warn!("Failed to set GitHub URL: {}", e);
        }
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
    println!("  Open http://localhost:5173");
    println!();

    Ok(())
}

