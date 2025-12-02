use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fs::File;
use verus_syn::parse_file;
use verus_syn::{visit::Visit, Expr};

#[derive(Debug, Deserialize, Clone)]
struct AtomWithMetrics {
    identifier: String,
    statement_type: String,
    deps: Vec<String>,
    body: String,
    display_name: String,
    full_path: String,
    #[allow(dead_code)]
    relative_path: String,
    #[allow(dead_code)]
    file_name: String,
    #[allow(dead_code)]
    parent_folder: String,
    #[allow(dead_code)]
    metrics: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct AtomWithProofMetrics {
    identifier: String,
    statement_type: String,
    deps: Vec<String>,
    body: String,
    display_name: String,
    full_path: String,
    relative_path: String,
    file_name: String,
    parent_folder: String,
    metrics: serde_json::Value,
    proof_metrics: Option<ProofMetrics>,
}

#[derive(Debug, Serialize, Clone)]
struct ProofMetrics {
    /// Direct Halstead metrics for proof block only
    direct_proof_halstead: HalsteadCounts,
    /// Transitive Halstead metrics (proof + all called lemmas)
    transitive_proof_halstead: HalsteadCounts,
    /// Lemmas called directly in proof block
    direct_lemmas: Vec<String>,
    /// All lemmas called transitively
    transitive_lemmas: Vec<String>,
    /// Maximum depth of lemma call chain
    proof_depth: usize,
    /// Parse errors if any
    #[serde(skip_serializing_if = "Option::is_none")]
    parse_error: Option<String>,
}

#[derive(Debug, Serialize, Clone, Default)]
struct HalsteadCounts {
    /// n1: Unique operators
    n1: usize,
    /// N1: Total operators
    n1_total: usize,
    /// n2: Unique operands
    n2: usize,
    /// N2: Total operands
    n2_total: usize,
    /// N: Total tokens (N1 + N2)
    length: usize,
    /// Difficulty: (n1/2) * (N2/n2)
    difficulty: f64,
    /// Volume: N * log2(n)
    volume: f64,
    /// Effort: difficulty * volume
    effort: f64,
}

impl HalsteadCounts {
    #[allow(dead_code)]
    fn from_visitor(visitor: &HalsteadVisitor) -> Self {
        let n1 = visitor.unique_operators.len();
        let n1_total = visitor.operators.len();
        let n2 = visitor.unique_operands.len();
        let n2_total = visitor.operands.len();
        let length = n1_total + n2_total;

        let vocabulary = n1 + n2;
        let difficulty = if n2 > 0 {
            (n1 as f64 / 2.0) * (n2_total as f64 / n2 as f64)
        } else {
            0.0
        };

        let volume = if vocabulary > 0 {
            (length as f64) * (vocabulary as f64).log2()
        } else {
            0.0
        };

        let effort = difficulty * volume;

        Self {
            n1,
            n1_total,
            n2,
            n2_total,
            length,
            difficulty,
            volume,
            effort,
        }
    }

    /// Aggregate counts by summing totals and taking union of unique elements
    fn aggregate(visitors: &[HalsteadVisitor]) -> Self {
        let mut all_operators = HashSet::new();
        let mut all_operands = HashSet::new();
        let mut total_operators = 0;
        let mut total_operands = 0;

        for visitor in visitors {
            all_operators.extend(visitor.unique_operators.iter().cloned());
            all_operands.extend(visitor.unique_operands.iter().cloned());
            total_operators += visitor.operators.len();
            total_operands += visitor.operands.len();
        }

        let n1 = all_operators.len();
        let n2 = all_operands.len();
        let length = total_operators + total_operands;
        let vocabulary = n1 + n2;

        let difficulty = if n2 > 0 {
            (n1 as f64 / 2.0) * (total_operands as f64 / n2 as f64)
        } else {
            0.0
        };

        let volume = if vocabulary > 0 {
            (length as f64) * (vocabulary as f64).log2()
        } else {
            0.0
        };

        let effort = difficulty * volume;

        Self {
            n1,
            n1_total: total_operators,
            n2,
            n2_total: total_operands,
            length,
            difficulty,
            volume,
            effort,
        }
    }
}

#[derive(Default)]
struct HalsteadVisitor {
    operators: Vec<String>,
    operands: Vec<String>,
    unique_operators: HashSet<String>,
    unique_operands: HashSet<String>,
}

impl<'ast> Visit<'ast> for HalsteadVisitor {
    fn visit_expr(&mut self, expr: &'ast Expr) {
        match expr {
            Expr::Binary(bin) => {
                let op = quote::ToTokens::to_token_stream(&bin.op).to_string();
                self.operators.push(op.clone());
                self.unique_operators.insert(op);
                self.visit_expr(&bin.left);
                self.visit_expr(&bin.right);
            }
            Expr::Unary(un) => {
                let op = quote::ToTokens::to_token_stream(&un.op).to_string();
                self.operators.push(op.clone());
                self.unique_operators.insert(op);
                self.visit_expr(&un.expr);
            }
            Expr::Path(path) => {
                let name = path
                    .path
                    .segments
                    .iter()
                    .map(|seg| seg.ident.to_string())
                    .collect::<Vec<_>>()
                    .join("::");
                self.operands.push(name.clone());
                self.unique_operands.insert(name);
            }
            Expr::Lit(lit) => {
                use quote::ToTokens;
                let lit_str = lit.lit.to_token_stream().to_string();
                self.operands.push(lit_str.clone());
                self.unique_operands.insert(lit_str);
            }
            Expr::Call(_) => {
                self.operators.push("call".to_string());
                self.unique_operators.insert("call".to_string());
                verus_syn::visit::visit_expr(self, expr);
            }
            Expr::MethodCall(_) => {
                self.operators.push("method_call".to_string());
                self.unique_operators.insert("method_call".to_string());
                verus_syn::visit::visit_expr(self, expr);
            }
            _ => verus_syn::visit::visit_expr(self, expr),
        }
    }
}

/// Extract proof blocks from function body
fn extract_proof_blocks(body: &str) -> Result<Vec<String>, String> {
    // Try to parse as a complete file
    let wrapped = format!("fn dummy() {{\n{}\n}}", body);

    match parse_file(&wrapped) {
        Ok(_file) => {
            // Walk the AST looking for proof blocks
            // In Verus, proof blocks are typically:
            // - proof { ... }
            // - assert(...) by { ... }
            // We'll extract these using simple pattern matching for now

            // TODO: Use proper AST visitor
            // For now, use regex as a fallback
            extract_proof_blocks_regex(body)
        }
        Err(_) => {
            // Fallback to regex-based extraction
            extract_proof_blocks_regex(body)
        }
    }
}

/// Fallback: Extract proof blocks using regex
fn extract_proof_blocks_regex(body: &str) -> Result<Vec<String>, String> {
    let mut proof_blocks = Vec::new();

    // Find "proof {" blocks
    let chars: Vec<char> = body.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // Look for "proof {"
        if i + 6 < chars.len() {
            let window: String = chars[i..i + 6].iter().collect();
            if window == "proof " || window == "proof{" {
                // Find the matching }
                let start = i + window.len();
                if let Some(block) = extract_balanced_block(&chars, start) {
                    proof_blocks.push(block);
                }
            }
        }
        i += 1;
    }

    Ok(proof_blocks)
}

/// Extract a balanced {...} block starting from position
fn extract_balanced_block(chars: &[char], start: usize) -> Option<String> {
    let mut depth = 0;
    let mut i = start;
    let mut block_start = None;

    while i < chars.len() {
        match chars[i] {
            '{' => {
                if depth == 0 {
                    block_start = Some(i + 1);
                }
                depth += 1;
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    if let Some(start_pos) = block_start {
                        return Some(chars[start_pos..i].iter().collect());
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }

    None
}

/// Extract lemma calls from code
fn extract_lemma_calls(code: &str) -> Vec<String> {
    let mut calls = Vec::new();

    // Try to parse and find function calls
    // For now, use a simple heuristic: look for identifiers followed by (
    let re = regex::Regex::new(r"\b(lemma_[a-zA-Z0-9_]+)\s*\(").unwrap();
    for cap in re.captures_iter(code) {
        if let Some(name) = cap.get(1) {
            calls.push(name.as_str().to_string());
        }
    }

    calls
}

/// Compute transitive proof metrics
fn compute_transitive_metrics(
    atom: &AtomWithMetrics,
    atoms_map: &HashMap<String, AtomWithMetrics>,
    visited: &mut HashSet<String>,
    depth: usize,
    max_depth: usize,
) -> (Vec<HalsteadVisitor>, Vec<String>, usize) {
    if depth > max_depth || visited.contains(&atom.identifier) {
        return (Vec::new(), Vec::new(), depth);
    }

    visited.insert(atom.identifier.clone());

    let mut all_visitors = Vec::new();
    let mut all_lemmas = Vec::new();
    let mut max_observed_depth = depth;

    // Extract proof blocks from this function
    if let Ok(proof_blocks) = extract_proof_blocks(&atom.body) {
        for block in &proof_blocks {
            // Compute Halstead for this proof block
            let wrapped = format!("fn dummy() {{ {} }}", block);
            if let Ok(file) = parse_file(&wrapped) {
                let mut visitor = HalsteadVisitor::default();
                // Visit all items in the file
                for item in &file.items {
                    verus_syn::visit::visit_item(&mut visitor, item);
                }
                all_visitors.push(visitor);
            }

            // Find lemma calls in this proof block
            let lemma_calls = extract_lemma_calls(block);

            // Recurse into each called lemma
            for lemma_name in lemma_calls {
                all_lemmas.push(lemma_name.clone());

                // Find the lemma in our atoms
                // Try to match by display_name or identifier
                for (_, callee_atom) in atoms_map.iter() {
                    if callee_atom.display_name.contains(&lemma_name)
                        || callee_atom.identifier.contains(&lemma_name)
                    {
                        // Check if it's a proof function or lemma
                        if callee_atom.statement_type.contains("proof")
                            || callee_atom.display_name.starts_with("lemma_")
                        {
                            let (callee_visitors, callee_lemmas, callee_depth) =
                                compute_transitive_metrics(
                                    callee_atom,
                                    atoms_map,
                                    visited,
                                    depth + 1,
                                    max_depth,
                                );

                            all_visitors.extend(callee_visitors);
                            all_lemmas.extend(callee_lemmas);
                            max_observed_depth = max_observed_depth.max(callee_depth);
                            break;
                        }
                    }
                }
            }
        }
    }

    (all_visitors, all_lemmas, max_observed_depth)
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} <input_atoms_json> <output_atoms_json>", args[0]);
        eprintln!();
        eprintln!(
            "Computes Halstead metrics for proof blocks including transitive lemma dependencies."
        );
        eprintln!();
        eprintln!("Example:");
        eprintln!("  {} \\", args[0]);
        eprintln!("    curve_dalek_atoms_with_spec_halstead_v18.json \\");
        eprintln!("    curve_dalek_atoms_with_proof_metrics.json");
        std::process::exit(1);
    }

    let input_path = &args[1];
    let output_path = &args[2];

    println!("Loading atoms from {}...", input_path);
    let file = File::open(input_path)?;
    let atoms: Vec<AtomWithMetrics> = serde_json::from_reader(file)?;
    println!("  Loaded {} functions", atoms.len());

    // Build lookup map
    let mut atoms_map: HashMap<String, AtomWithMetrics> = HashMap::new();
    for atom in &atoms {
        atoms_map.insert(atom.identifier.clone(), atom.clone());
    }

    println!("Computing proof metrics...");
    let mut atoms_with_proof: Vec<AtomWithProofMetrics> = Vec::new();
    let mut processed = 0;
    let mut with_proofs = 0;
    let max_depth = 10; // Limit recursion depth

    for atom in &atoms {
        processed += 1;
        if processed % 100 == 0 {
            print!("\r  Processed {}/{} functions...", processed, atoms.len());
        }

        // Try to extract and compute proof metrics
        let proof_metrics = if let Ok(proof_blocks) = extract_proof_blocks(&atom.body) {
            if !proof_blocks.is_empty() {
                with_proofs += 1;

                // Compute direct proof metrics
                let mut direct_visitors = Vec::new();
                let mut direct_lemmas = Vec::new();

                for block in &proof_blocks {
                    let wrapped = format!("fn dummy() {{ {} }}", block);
                    if let Ok(file) = parse_file(&wrapped) {
                        let mut visitor = HalsteadVisitor::default();
                        for item in &file.items {
                            verus_syn::visit::visit_item(&mut visitor, item);
                        }
                        direct_visitors.push(visitor);
                    }

                    direct_lemmas.extend(extract_lemma_calls(block));
                }

                let direct_halstead = HalsteadCounts::aggregate(&direct_visitors);

                // Compute transitive metrics
                let mut visited = HashSet::new();
                let (transitive_visitors, mut transitive_lemmas, proof_depth) =
                    compute_transitive_metrics(atom, &atoms_map, &mut visited, 0, max_depth);

                // Remove duplicates from transitive_lemmas
                transitive_lemmas.sort();
                transitive_lemmas.dedup();

                let transitive_halstead = HalsteadCounts::aggregate(&transitive_visitors);

                Some(ProofMetrics {
                    direct_proof_halstead: direct_halstead,
                    transitive_proof_halstead: transitive_halstead,
                    direct_lemmas: direct_lemmas.clone(),
                    transitive_lemmas,
                    proof_depth,
                    parse_error: None,
                })
            } else {
                None
            }
        } else {
            None
        };

        atoms_with_proof.push(AtomWithProofMetrics {
            identifier: atom.identifier.clone(),
            statement_type: atom.statement_type.clone(),
            deps: atom.deps.clone(),
            body: atom.body.clone(),
            display_name: atom.display_name.clone(),
            full_path: atom.full_path.clone(),
            relative_path: atom.relative_path.clone(),
            file_name: atom.file_name.clone(),
            parent_folder: atom.parent_folder.clone(),
            metrics: atom.metrics.clone(),
            proof_metrics,
        });
    }

    println!("\r  Processed {}/{} functions    ", processed, atoms.len());

    println!("Writing output to {}...", output_path);
    let output_file = File::create(output_path)?;
    serde_json::to_writer_pretty(output_file, &atoms_with_proof)?;

    println!("✓ Done!");
    println!();
    println!("Summary:");
    println!("  Total functions: {}", atoms_with_proof.len());
    println!("  With proof blocks: {}", with_proofs);
    println!(
        "  With transitive proof metrics: {}",
        atoms_with_proof
            .iter()
            .filter(|a| a.proof_metrics.is_some())
            .count()
    );

    Ok(())
}
