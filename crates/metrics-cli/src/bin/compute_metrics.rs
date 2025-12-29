//! Compute Verus specification metrics using verus_syn AST parsing.
//!
//! Uses verus_syn to parse entire function items, extracting specs directly
//! from the AST instead of using string manipulation.
//!
//! Benefits:
//! - Robust parsing (no edge cases from string matching)
//! - Function mode (exec/proof/spec) extracted automatically
//! - Specs already parsed as expressions
//! - Clean, maintainable code

use quote::ToTokens;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use verus_syn::visit::Visit;
use verus_syn::{Expr, Item, ItemFn, ImplItem, TraitItem};

// Input format from write_atoms
#[derive(Debug, Deserialize)]
struct Atom {
    identifier: String,
    statement_type: String,
    deps: Vec<String>,
    body: String,
    display_name: String,
    full_path: String,
    relative_path: String,
    file_name: String,
    parent_folder: String,
}

// Output format with metrics
#[derive(Debug, Serialize)]
struct AtomWithMetrics {
    identifier: String,
    statement_type: String,
    deps: Vec<String>,
    body: String,
    display_name: String,
    full_path: String,
    relative_path: String,
    file_name: String,
    parent_folder: String,
    metrics: FunctionMetrics,
}

#[derive(Debug, Serialize)]
struct SpecHalsteadMetrics {
    text: String,
    halstead_length: Option<usize>,
    halstead_difficulty: Option<f64>,
    halstead_effort: Option<f64>,
    halstead_vocabulary: Option<usize>,
    halstead_volume: Option<f64>,
    unique_operators: Option<usize>,
    total_operators: Option<usize>,
    unique_operands: Option<usize>,
    total_operands: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parse_error: Option<String>,
}

#[derive(Debug, Serialize)]
struct FunctionMetrics {
    /// Function mode: exec, proof, or spec
    function_mode: String,
    requires_count: usize,
    requires_lengths: Vec<usize>,
    requires_specs: Vec<SpecHalsteadMetrics>,
    ensures_count: usize,
    ensures_lengths: Vec<usize>,
    ensures_specs: Vec<SpecHalsteadMetrics>,
    decreases_count: usize,
    decreases_specs: Vec<SpecHalsteadMetrics>,
    body_length: usize,
    operators: HashMap<String, usize>,
}

impl Default for FunctionMetrics {
    fn default() -> Self {
        Self {
            function_mode: "exec".to_string(),
            requires_count: 0,
            requires_lengths: Vec::new(),
            requires_specs: Vec::new(),
            ensures_count: 0,
            ensures_lengths: Vec::new(),
            ensures_specs: Vec::new(),
            decreases_count: 0,
            decreases_specs: Vec::new(),
            body_length: 0,
            operators: HashMap::new(),
        }
    }
}

// ============================================================================
// Halstead Metrics Computation (from verus_syn Expr)
// ============================================================================

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
                let op = bin.op.to_token_stream().to_string();
                self.operators.push(op.clone());
                self.unique_operators.insert(op);
                self.visit_expr(&bin.left);
                self.visit_expr(&bin.right);
            }
            Expr::Unary(un) => {
                let op = un.op.to_token_stream().to_string();
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
                let lit_str = lit.lit.to_token_stream().to_string();
                self.operands.push(lit_str.clone());
                self.unique_operands.insert(lit_str);
            }
            Expr::Call(call) => {
                self.operators.push("call".to_string());
                self.unique_operators.insert("call".to_string());
                self.visit_expr(&call.func);
                for arg in &call.args {
                    self.visit_expr(arg);
                }
            }
            Expr::MethodCall(method) => {
                let method_name = method.method.to_string();
                self.operators.push(method_name.clone());
                self.unique_operators.insert(method_name);
                self.visit_expr(&method.receiver);
                for arg in &method.args {
                    self.visit_expr(arg);
                }
            }
            Expr::Field(field) => {
                self.operators.push(".".to_string());
                self.unique_operators.insert(".".to_string());
                let field_name = match &field.member {
                    verus_syn::Member::Named(ident) => ident.to_string(),
                    verus_syn::Member::Unnamed(index) => index.index.to_string(),
                };
                self.operands.push(field_name.clone());
                self.unique_operands.insert(field_name);
                self.visit_expr(&field.base);
            }
            Expr::Index(index) => {
                self.operators.push("[]".to_string());
                self.unique_operators.insert("[]".to_string());
                self.visit_expr(&index.expr);
                self.visit_expr(&index.index);
            }
            Expr::Paren(paren) => {
                self.operators.push("()".to_string());
                self.unique_operators.insert("()".to_string());
                self.visit_expr(&paren.expr);
            }
            Expr::Cast(cast) => {
                self.operators.push("as".to_string());
                self.unique_operators.insert("as".to_string());
                self.visit_expr(&cast.expr);
            }
            Expr::Reference(reference) => {
                self.operators.push("&".to_string());
                self.unique_operators.insert("&".to_string());
                self.visit_expr(&reference.expr);
            }
            _ => verus_syn::visit::visit_expr(self, expr),
        }
    }
}

/// Compute Halstead metrics from a verus_syn Expr
fn compute_halstead_from_expr(expr: &Expr) -> SpecHalsteadMetrics {
    let mut visitor = HalsteadVisitor::default();
    visitor.visit_expr(expr);

    let n1 = visitor.unique_operators.len();
    let n2 = visitor.unique_operands.len();
    let n1_total = visitor.operators.len();
    let n2_total = visitor.operands.len();

    let length = n1_total + n2_total;
    let vocabulary = n1 + n2;

    let difficulty = if n2 == 0 {
        0.0
    } else {
        (n1 as f64 / 2.0) * (n2_total as f64 / n2 as f64)
    };

    let volume = if vocabulary == 0 {
        0.0
    } else {
        length as f64 * (vocabulary as f64).log2()
    };

    let effort = difficulty * volume;

    SpecHalsteadMetrics {
        text: expr.to_token_stream().to_string(),
        halstead_length: Some(length),
        halstead_difficulty: Some(difficulty),
        halstead_effort: Some(effort),
        halstead_vocabulary: Some(vocabulary),
        halstead_volume: Some(volume),
        unique_operators: Some(n1),
        total_operators: Some(n1_total),
        unique_operands: Some(n2),
        total_operands: Some(n2_total),
        parse_error: None,
    }
}

// ============================================================================
// Function Mode Extraction
// ============================================================================

fn fn_mode_to_string(mode: &verus_syn::FnMode) -> String {
    match mode {
        verus_syn::FnMode::Default => "exec".to_string(),
        verus_syn::FnMode::Spec(_) => "spec".to_string(),
        verus_syn::FnMode::SpecChecked(_) => "spec".to_string(),
        verus_syn::FnMode::Proof(_) => "proof".to_string(),
        verus_syn::FnMode::ProofAxiom(_) => "proof".to_string(),
        verus_syn::FnMode::Exec(_) => "exec".to_string(),
    }
}

// ============================================================================
// Main Metrics Extraction from ItemFn
// ============================================================================

/// Extract metrics from an ItemFn using verus_syn's structured parsing
fn extract_metrics_from_item_fn(item_fn: &ItemFn) -> FunctionMetrics {
    let mut metrics = FunctionMetrics::default();

    // Extract function mode
    metrics.function_mode = fn_mode_to_string(&item_fn.sig.mode);

    // Extract requires clauses
    if let Some(requires) = &item_fn.sig.spec.requires {
        let exprs: Vec<&Expr> = requires.exprs.exprs.iter().collect();
        metrics.requires_count = exprs.len();
        metrics.requires_lengths = exprs.iter().map(|e| e.to_token_stream().to_string().len()).collect();
        metrics.requires_specs = exprs.iter().map(|e| compute_halstead_from_expr(e)).collect();
    }

    // Extract ensures clauses
    if let Some(ensures) = &item_fn.sig.spec.ensures {
        let exprs: Vec<&Expr> = ensures.exprs.exprs.iter().collect();
        metrics.ensures_count = exprs.len();
        metrics.ensures_lengths = exprs.iter().map(|e| e.to_token_stream().to_string().len()).collect();
        metrics.ensures_specs = exprs.iter().map(|e| compute_halstead_from_expr(e)).collect();
    }

    // Extract decreases clauses
    if let Some(decreases) = &item_fn.sig.spec.decreases {
        let exprs: Vec<&Expr> = decreases.decreases.exprs.exprs.iter().collect();
        metrics.decreases_count = exprs.len();
        metrics.decreases_specs = exprs.iter().map(|e| compute_halstead_from_expr(e)).collect();
    }

    // Compute body length (the actual function block)
    metrics.body_length = item_fn.block.to_token_stream().to_string().len();

    // Count operators in body
    let mut body_visitor = HalsteadVisitor::default();
    for stmt in &item_fn.block.stmts {
        verus_syn::visit::visit_stmt(&mut body_visitor, stmt);
    }
    for (op, _) in body_visitor.unique_operators.iter().zip(body_visitor.operators.iter()) {
        *metrics.operators.entry(op.clone()).or_insert(0) += 1;
    }

    metrics
}

/// Try to parse body as different Verus item types
fn compute_function_metrics(body: &str) -> FunctionMetrics {
    // Attempt 1: Parse as standalone ItemFn
    if let Ok(item_fn) = verus_syn::parse_str::<ItemFn>(body) {
        return extract_metrics_from_item_fn(&item_fn);
    }

    // Attempt 2: Parse as Item (covers more cases)
    if let Ok(item) = verus_syn::parse_str::<Item>(body) {
        if let Item::Fn(item_fn) = item {
            return extract_metrics_from_item_fn(&item_fn);
        }
    }

    // Attempt 3: Parse as ImplItemFn (method inside impl block)
    if let Ok(impl_item) = verus_syn::parse_str::<ImplItem>(body) {
        if let ImplItem::Fn(impl_fn) = impl_item {
            let mut metrics = FunctionMetrics::default();
            metrics.function_mode = fn_mode_to_string(&impl_fn.sig.mode);

            if let Some(requires) = &impl_fn.sig.spec.requires {
                let exprs: Vec<&Expr> = requires.exprs.exprs.iter().collect();
                metrics.requires_count = exprs.len();
                metrics.requires_lengths = exprs.iter().map(|e| e.to_token_stream().to_string().len()).collect();
                metrics.requires_specs = exprs.iter().map(|e| compute_halstead_from_expr(e)).collect();
            }

            if let Some(ensures) = &impl_fn.sig.spec.ensures {
                let exprs: Vec<&Expr> = ensures.exprs.exprs.iter().collect();
                metrics.ensures_count = exprs.len();
                metrics.ensures_lengths = exprs.iter().map(|e| e.to_token_stream().to_string().len()).collect();
                metrics.ensures_specs = exprs.iter().map(|e| compute_halstead_from_expr(e)).collect();
            }

            if let Some(decreases) = &impl_fn.sig.spec.decreases {
                let exprs: Vec<&Expr> = decreases.decreases.exprs.exprs.iter().collect();
                metrics.decreases_count = exprs.len();
                metrics.decreases_specs = exprs.iter().map(|e| compute_halstead_from_expr(e)).collect();
            }

            metrics.body_length = impl_fn.block.to_token_stream().to_string().len();
            return metrics;
        }
    }

    // Attempt 4: Parse as TraitItemFn
    if let Ok(trait_item) = verus_syn::parse_str::<TraitItem>(body) {
        if let TraitItem::Fn(trait_fn) = trait_item {
            let mut metrics = FunctionMetrics::default();
            metrics.function_mode = fn_mode_to_string(&trait_fn.sig.mode);

            if let Some(requires) = &trait_fn.sig.spec.requires {
                let exprs: Vec<&Expr> = requires.exprs.exprs.iter().collect();
                metrics.requires_count = exprs.len();
                metrics.requires_lengths = exprs.iter().map(|e| e.to_token_stream().to_string().len()).collect();
                metrics.requires_specs = exprs.iter().map(|e| compute_halstead_from_expr(e)).collect();
            }

            if let Some(ensures) = &trait_fn.sig.spec.ensures {
                let exprs: Vec<&Expr> = ensures.exprs.exprs.iter().collect();
                metrics.ensures_count = exprs.len();
                metrics.ensures_lengths = exprs.iter().map(|e| e.to_token_stream().to_string().len()).collect();
                metrics.ensures_specs = exprs.iter().map(|e| compute_halstead_from_expr(e)).collect();
            }

            if let Some(decreases) = &trait_fn.sig.spec.decreases {
                let exprs: Vec<&Expr> = decreases.decreases.exprs.exprs.iter().collect();
                metrics.decreases_count = exprs.len();
                metrics.decreases_specs = exprs.iter().map(|e| compute_halstead_from_expr(e)).collect();
            }

            if let Some(block) = &trait_fn.default {
                metrics.body_length = block.to_token_stream().to_string().len();
            }
            return metrics;
        }
    }

    // Fallback: Return default metrics if parsing fails
    // This handles non-function bodies or incomplete fragments
    FunctionMetrics {
        function_mode: "unknown".to_string(),
        body_length: body.len(),
        ..Default::default()
    }
}

// ============================================================================
// Main
// ============================================================================

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!(
            "Usage: {} <input_atoms_json> <output_metrics_json>",
            args[0]
        );
        eprintln!("\nUses verus_syn AST parsing for robust spec extraction.");
        eprintln!("\nExample:");
        eprintln!(
            "  {} curve_dalek_atoms.json curve_dalek_atoms_with_metrics.json",
            args[0]
        );
        std::process::exit(1);
    }

    let input_path = &args[1];
    let output_path = &args[2];

    println!("Loading atoms from {}...", input_path);
    let content = fs::read_to_string(input_path).unwrap_or_else(|e| {
        eprintln!("Failed to read input file: {}", e);
        std::process::exit(1);
    });

    let atoms: Vec<Atom> = serde_json::from_str(&content).unwrap_or_else(|e| {
        eprintln!("Failed to parse input JSON: {}", e);
        std::process::exit(1);
    });

    println!("  Loaded {} functions", atoms.len());

    println!("Computing metrics (using verus_syn AST parsing)...");
    let atoms_with_metrics: Vec<AtomWithMetrics> = atoms
        .iter()
        .map(|atom| {
            let metrics = compute_function_metrics(&atom.body);
            AtomWithMetrics {
                identifier: atom.identifier.clone(),
                statement_type: atom.statement_type.clone(),
                deps: atom.deps.clone(),
                body: atom.body.clone(),
                display_name: atom.display_name.clone(),
                full_path: atom.full_path.clone(),
                relative_path: atom.relative_path.clone(),
                file_name: atom.file_name.clone(),
                parent_folder: atom.parent_folder.clone(),
                metrics,
            }
        })
        .collect();

    println!("Writing output to {}...", output_path);
    let output_json = serde_json::to_string_pretty(&atoms_with_metrics).unwrap_or_else(|e| {
        eprintln!("Failed to serialize output: {}", e);
        std::process::exit(1);
    });

    fs::write(output_path, output_json).unwrap_or_else(|e| {
        eprintln!("Failed to write output file: {}", e);
        std::process::exit(1);
    });

    println!("✓ Done!");

    // Print summary statistics
    let with_requires: usize = atoms_with_metrics
        .iter()
        .filter(|a| a.metrics.requires_count > 0)
        .count();
    let with_ensures: usize = atoms_with_metrics
        .iter()
        .filter(|a| a.metrics.ensures_count > 0)
        .count();
    let with_decreases: usize = atoms_with_metrics
        .iter()
        .filter(|a| a.metrics.decreases_count > 0)
        .count();

    // Count by function mode
    let exec_count = atoms_with_metrics
        .iter()
        .filter(|a| a.metrics.function_mode == "exec")
        .count();
    let proof_count = atoms_with_metrics
        .iter()
        .filter(|a| a.metrics.function_mode == "proof")
        .count();
    let spec_count = atoms_with_metrics
        .iter()
        .filter(|a| a.metrics.function_mode == "spec")
        .count();
    let unknown_count = atoms_with_metrics
        .iter()
        .filter(|a| a.metrics.function_mode == "unknown")
        .count();

    println!("\nSummary:");
    println!("  Total functions: {}", atoms_with_metrics.len());
    println!("  Function modes:");
    println!("    - exec: {}", exec_count);
    println!("    - proof: {}", proof_count);
    println!("    - spec: {}", spec_count);
    println!("    - unknown (parse failed): {}", unknown_count);
    println!("  Specs found:");
    println!("    - With requires: {}", with_requires);
    println!("    - With ensures: {}", with_ensures);
    println!("    - With decreases: {}", with_decreases);

    if let Some(example) = atoms_with_metrics
        .iter()
        .find(|a| a.metrics.requires_count > 0 || a.metrics.ensures_count > 0)
    {
        println!("\nExample function with specs:");
        println!("  Name: {}", example.display_name);
        println!("  Mode: {}", example.metrics.function_mode);
        println!("  Requires: {}", example.metrics.requires_count);
        println!("  Ensures: {}", example.metrics.ensures_count);
        println!("  Body length: {}", example.metrics.body_length);

        if !example.metrics.requires_specs.is_empty() {
            if let Some(first_req) = example.metrics.requires_specs.first() {
                println!("\n  First requires clause:");
                println!("    Text: {}", first_req.text);
                if let Some(len) = first_req.halstead_length {
                    println!("    Halstead length: {}", len);
                }
                if let Some(diff) = first_req.halstead_difficulty {
                    println!("    Halstead difficulty: {:.2}", diff);
                }
                if let Some(effort) = first_req.halstead_effort {
                    println!("    Halstead effort: {:.2}", effort);
                }
            }
        }
    }
}

