//! Halstead complexity metrics for Verus specifications
//!
//! This module computes Halstead metrics (length, difficulty, effort) for
//! pre/postconditions in Verus-verified Rust code.
//!
//! ## Refactored to use verus_syn
//!
//! This module now uses `verus_syn` (Verus-extended parser) instead of `syn`,
//! which natively handles Verus-specific syntax like:
//! - `@` ghost operator
//! - `&&&`, `|||` conjunctions/disjunctions  
//! - `=~=` equivalence operator
//! - `forall|`, `exists|` quantifiers
//! - `==>` implication
//! - `#![trigger]` annotations
//! - `old()` expressions

use quote::ToTokens;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use verus_syn::{visit::Visit, Expr};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SpecHalsteadMetrics {
    /// Total number of operators + operands (N = N1 + N2)
    pub halstead_length: usize,

    /// Size of vocabulary (n = n1 + n2)
    pub vocabulary: usize,

    /// Halstead difficulty: (n1/2) × (N2/n2)
    pub difficulty: f64,

    /// Halstead volume: N × log2(n)
    pub volume: f64,

    /// Halstead effort: difficulty × volume
    pub effort: f64,

    /// Number of unique operators
    pub n1_unique_operators: usize,

    /// Number of unique operands
    pub n2_unique_operands: usize,

    /// Total operator occurrences
    pub n1_total_operators: usize,

    /// Total operand occurrences
    pub n2_total_operands: usize,
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
                // Count binary operators: +, -, *, ==, <, &&, etc.
                let op = bin.op.to_token_stream().to_string();
                self.operators.push(op.clone());
                self.unique_operators.insert(op);

                // Visit operands recursively
                self.visit_expr(&bin.left);
                self.visit_expr(&bin.right);
            }
            Expr::Unary(un) => {
                // Count unary operators: !, -, *
                let op = un.op.to_token_stream().to_string();
                self.operators.push(op.clone());
                self.unique_operators.insert(op);

                self.visit_expr(&un.expr);
            }
            Expr::Path(path) => {
                // Count variable names, constants, type names as operands
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
                // Count literals as operands
                let lit_str = match &lit.lit {
                    verus_syn::Lit::Str(s) => format!("\"{}\"", s.value()),
                    verus_syn::Lit::ByteStr(b) => format!("{:?}", b.value()),
                    verus_syn::Lit::CStr(c) => format!("{:?}", c.value()),
                    verus_syn::Lit::Byte(b) => format!("{}", b.value()),
                    verus_syn::Lit::Char(c) => format!("'{}'", c.value()),
                    verus_syn::Lit::Int(i) => i.to_string(),
                    verus_syn::Lit::Float(f) => f.to_string(),
                    verus_syn::Lit::Bool(b) => b.value().to_string(),
                    verus_syn::Lit::Verbatim(v) => v.to_string(),
                    _ => "literal".to_string(),
                };
                self.operands.push(lit_str.clone());
                self.unique_operands.insert(lit_str);
            }
            Expr::Call(call) => {
                // Count function calls as operators
                self.operators.push("call".to_string());
                self.unique_operators.insert("call".to_string());

                self.visit_expr(&call.func);
                for arg in &call.args {
                    self.visit_expr(arg);
                }
            }
            Expr::MethodCall(method) => {
                // Count method calls as operators
                let method_name = method.method.to_string();
                self.operators.push(method_name.clone());
                self.unique_operators.insert(method_name);

                self.visit_expr(&method.receiver);
                for arg in &method.args {
                    self.visit_expr(arg);
                }
            }
            Expr::Field(field) => {
                // Count field access as operator
                self.operators.push(".".to_string());
                self.unique_operators.insert(".".to_string());

                // Field name is an operand
                let field_name = match &field.member {
                    verus_syn::Member::Named(ident) => ident.to_string(),
                    verus_syn::Member::Unnamed(index) => index.index.to_string(),
                };
                self.operands.push(field_name.clone());
                self.unique_operands.insert(field_name);

                self.visit_expr(&field.base);
            }
            Expr::Index(index) => {
                // Count array indexing as operator
                self.operators.push("[]".to_string());
                self.unique_operators.insert("[]".to_string());

                self.visit_expr(&index.expr);
                self.visit_expr(&index.index);
            }
            Expr::Paren(paren) => {
                // Count parentheses as operators
                self.operators.push("()".to_string());
                self.unique_operators.insert("()".to_string());

                self.visit_expr(&paren.expr);
            }
            Expr::Cast(cast) => {
                // Count casts as operators
                self.operators.push("as".to_string());
                self.unique_operators.insert("as".to_string());

                self.visit_expr(&cast.expr);
                // Type is counted when we visit it
            }
            Expr::Reference(reference) => {
                // Count reference operator
                self.operators.push("&".to_string());
                self.unique_operators.insert("&".to_string());

                self.visit_expr(&reference.expr);
            }
            // For other expression types, use default traversal
            _ => verus_syn::visit::visit_expr(self, expr),
        }
    }
}

impl HalsteadVisitor {
    fn compute_metrics(&self) -> SpecHalsteadMetrics {
        let n1 = self.unique_operators.len();
        let n2 = self.unique_operands.len();
        let n1_total = self.operators.len();
        let n2_total = self.operands.len();

        let length = n1_total + n2_total;
        let vocabulary = n1 + n2;

        // Handle edge cases
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
            halstead_length: length,
            vocabulary,
            difficulty,
            volume,
            effort,
            n1_unique_operators: n1,
            n2_unique_operands: n2,
            n1_total_operators: n1_total,
            n2_total_operands: n2_total,
        }
    }
}

// REMOVED: Type cast removal and chained comparison expansion
// These are now handled natively by verus_syn parser!
//
// Previously needed with standard `syn`:
// - remove_type_casts(): Stripped "as int", "as nat", etc.
// - expand_chained_comparisons(): Converted "0 <= i < 5" to "((0 <= i) && (i < 5))"
//
// verus_syn natively handles:
// - Type casts (as int, as nat, etc.)
// - Chained comparisons
// - All other Verus-specific syntax

/// Detect if a spec is primarily natural language prose rather than code
pub fn is_prose(spec: &str) -> bool {
    let spec_trimmed = spec.trim();

    // Empty or very short strings are not prose
    if spec_trimmed.len() < 10 {
        return false;
    }

    // Check for doc comment markers
    if spec_trimmed.starts_with("///") || spec_trimmed.starts_with("//!") {
        return true;
    }

    // Check for common prose patterns (English phrases without operators)
    let prose_indicators = [
        "However,",
        "Thus,",
        "Therefore,",
        "i.e.",
        "e.g.",
        "must be clear",
        "should be",
        "cannot add",
        "are swapped",
        "remain unchanged",
        "is equivalent to",
        "We have that",
        "only changing",
        "in either case",
        "returns if",
        "given input",
        "an inverse",
        "each coset",
        "is the multiplicative inverse",
        "is zero",
        "similarly for the",
    ];

    for pattern in &prose_indicators {
        if spec_trimmed.contains(pattern) {
            return true;
        }
    }

    // Check if it starts with a capitalized English word (not a type name)
    // Common English starter words that indicate prose
    let prose_starters = [
        "However",
        "Thus",
        "Therefore",
        "Given",
        "When",
        "If",
        "The ",
        "A ",
        "An ",
        "This ",
        "That ",
        "These ",
        "Those ",
        "We ",
        "It ",
        "As ",
        "For ",
        "In ",
        "On ",
        "At ",
    ];

    for starter in &prose_starters {
        if spec_trimmed.starts_with(starter) {
            // Make sure it's not followed by operators (which would indicate code)
            if !spec_trimmed.contains("==")
                && !spec_trimmed.contains("!=")
                && !spec_trimmed.contains("<=")
                && !spec_trimmed.contains(">=")
            {
                return true;
            }
        }
    }

    // Check for statement/comment only fragments
    if spec_trimmed.ends_with("*/") && !spec_trimmed.starts_with("/*") {
        return true; // End of comment fragment
    }

    // Check if it's mostly prose (ratio of letters to operators)
    let letter_count = spec_trimmed.chars().filter(|c| c.is_alphabetic()).count();
    let operator_chars = ['=', '<', '>', '!', '&', '|', '+', '-', '*', '/', '%'];
    let operator_count = spec_trimmed
        .chars()
        .filter(|c| operator_chars.contains(c))
        .count();

    // If we have lots of text but very few operators, likely prose
    if letter_count > 50 && operator_count < 3 {
        return true;
    }

    false
}

/// Preprocess specifications for verus_syn parsing
///
/// With verus_syn, we can handle Verus syntax natively, so preprocessing is minimal:
/// - Remove comments (they can interfere with parsing)
/// - Skip empty or incomplete expressions
fn preprocess_verus_spec(spec: &str) -> String {
    let mut result = spec.trim().to_string();

    // Remove comments (they can interfere with parsing)
    result = result
        .lines()
        .map(|line| {
            if let Some(pos) = line.find("//") {
                &line[..pos]
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();

    // Skip empty or incomplete expressions
    if result.is_empty() || result == "(" || result.ends_with("(") {
        return String::new();
    }

    // verus_syn handles natively:
    // - @ ghost operator
    // - &&&, ||| conjunctions/disjunctions
    // - =~= equivalence operator
    // - forall|, exists| quantifiers
    // - ==> implication
    // - #![trigger] annotations
    // - old() expressions
    // - Type casts (as int, as nat, etc.)
    // - Chained comparisons (potentially)

    result
}

/// Compute Halstead metrics for a specification string
pub fn analyze_spec(spec_text: &str) -> Result<SpecHalsteadMetrics, String> {
    if spec_text.is_empty() {
        return Ok(SpecHalsteadMetrics::default());
    }

    // Check for prose before preprocessing
    if is_prose(spec_text) {
        return Err(format!(
            "Skipped prose/documentation: '{}'",
            if spec_text.len() > 60 {
                format!("{}...", &spec_text[..60])
            } else {
                spec_text.to_string()
            }
        ));
    }

    // Preprocess Verus-specific syntax
    let preprocessed = preprocess_verus_spec(spec_text);

    // If preprocessing resulted in empty string (e.g., decreases clause), skip it
    if preprocessed.is_empty() {
        return Err(format!(
            "Skipped non-expression clause: '{}'",
            if spec_text.len() > 60 {
                format!("{}...", &spec_text[..60])
            } else {
                spec_text.to_string()
            }
        ));
    }

    // Try to parse as expression using verus_syn (handles Verus syntax natively)
    let expr: Expr = verus_syn::parse_str(&preprocessed)
        .map_err(|e| format!("Failed to parse spec '{}': {}", spec_text, e))?;

    // Visit AST and count tokens
    let mut visitor = HalsteadVisitor::default();
    visitor.visit_expr(&expr);

    Ok(visitor.compute_metrics())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_comparison() {
        let spec = "x < 10";
        let metrics = analyze_spec(spec).unwrap();

        // Operators: <
        // Operands: x, 10
        assert_eq!(metrics.n1_unique_operators, 1);
        assert_eq!(metrics.n2_unique_operands, 2);
        assert_eq!(metrics.halstead_length, 3);
    }

    #[test]
    fn test_complex_spec() {
        let spec = "x.0 < FIELD_MODULUS && y.0 < FIELD_MODULUS";
        let metrics = analyze_spec(spec).unwrap();

        println!("Metrics: {:?}", metrics);

        // Operators: ., <, &&, ., <
        // Operands: x, 0, FIELD_MODULUS, y, 0, FIELD_MODULUS
        assert!(metrics.halstead_length > 5);
        assert!(metrics.difficulty > 0.0);
    }

    #[test]
    fn test_verus_forall() {
        let spec = "forall|i: usize| i < len";
        let metrics = analyze_spec(spec).unwrap();

        // Should handle forall by converting to closure
        assert!(metrics.halstead_length > 0);
    }

    #[test]
    fn test_empty_spec() {
        let spec = "";
        let metrics = analyze_spec(spec).unwrap();

        assert_eq!(metrics.halstead_length, 0);
        assert_eq!(metrics.difficulty, 0.0);
    }

    #[test]
    fn test_chained_comparison() {
        let spec = "0 <= i < 5";
        let metrics = analyze_spec(spec).unwrap();

        // Should be expanded to: (0 <= i) && (i < 5)
        // Operators: <=, <, &&
        // Operands: 0, i (appears twice), 5
        println!("Chained comparison metrics: {:?}", metrics);
        assert!(metrics.halstead_length > 0);
        assert!(metrics.n1_unique_operators >= 2); // At least <= and <
    }

    #[test]
    fn test_chained_comparison_with_forall() {
        let spec = "forall|i: int| 0 <= i < 5 ==> limbs[i] < pow2(51)";
        let metrics = analyze_spec(spec).unwrap();

        // Complex spec with chained comparison inside forall
        println!("Forall with chained comparison: {:?}", metrics);
        assert!(metrics.halstead_length > 5);
    }

    #[test]
    fn test_chained_equality() {
        let spec = "a == b == c";
        let metrics = analyze_spec(spec).unwrap();

        // Should be expanded to: (a == b) && (b == c)
        println!("Chained equality metrics: {:?}", metrics);
        assert!(metrics.halstead_length > 0);
    }

    #[test]
    fn test_decreases_clause() {
        // "decreases i" is not a valid standalone expression in Rust/Verus,
        // because "decreases" is a Verus keyword only valid in function signatures.
        // The parser fails to parse this as an expression.
        let spec = "decreases i";
        let result = analyze_spec(spec);
        
        // This should fail to parse as an expression
        assert!(result.is_err());
    }

    #[test]
    fn test_incomplete_expression() {
        let spec = "(";
        let result = analyze_spec(spec);

        // Incomplete expressions should be skipped
        assert!(result.is_err());
    }

    #[test]
    fn test_trigger_annotation() {
        let spec = "forall|i: int| 0 <= i < 5 ==> #[trigger] limbs[i] < pow2(51)";
        let metrics = analyze_spec(spec).unwrap();

        // Should handle #[trigger] annotations by removing them
        println!("Spec with trigger: {:?}", metrics);
        assert!(metrics.halstead_length > 0);
    }

    #[test]
    fn test_chained_comparison_with_function_calls() {
        let spec = "0 < pow2(s) <= u64::MAX";
        let metrics = analyze_spec(spec).unwrap();

        // Should expand to: ((0 < pow2(s)) && (pow2(s) <= u64::MAX))
        println!("Chained comparison with function: {:?}", metrics);
        assert!(metrics.halstead_length > 5);
        assert!(metrics.n1_unique_operators >= 2); // At least < and <=
    }

    #[test]
    fn test_chained_comparison_with_path() {
        let spec = "1 <= value <= i32::MAX";
        let metrics = analyze_spec(spec).unwrap();

        // Should handle path expressions like i32::MAX
        // verus_syn parses chained comparisons - length is at least 5
        // (operators: <=, <=; operands: 1, value, i32::MAX)
        println!("Chained comparison with path: {:?}", metrics);
        assert!(metrics.halstead_length >= 5);
    }

    #[test]
    fn test_prose_detection_simple() {
        let spec = "scalars should be canonical";
        let result = analyze_spec(spec);

        // Should be skipped as prose
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Skipped"));
    }

    #[test]
    fn test_prose_detection_with_thus() {
        let spec = "two inversions.\n        // We have that X = xZ and X' = x'Z'. Thus";
        let result = analyze_spec(spec);

        // Should be skipped as prose
        assert!(result.is_err());
    }

    #[test]
    fn test_prose_detection_doc_comment() {
        let spec = "/// This is a doc comment explaining the function";
        let result = analyze_spec(spec);

        // Should be skipped as documentation
        assert!(result.is_err());
    }

    #[test]
    fn test_not_prose_with_operators() {
        let spec = "x > 0 && y > 0";
        let result = analyze_spec(spec);

        // Should NOT be detected as prose (has operators)
        assert!(result.is_ok());
    }

    #[test]
    fn test_prose_detection_starts_with_the() {
        let spec = "The value must be positive";
        let result = analyze_spec(spec);

        // Should be skipped as prose
        assert!(result.is_err());
    }

    #[test]
    fn test_prose_detection_ie() {
        let spec = "i.e.";
        let result = analyze_spec(spec);

        // Should be skipped as prose
        assert!(result.is_err());
    }

    #[test]
    fn test_prose_with_code_mixed() {
        let spec = "top bit must be clear\n            w == 4 ==> self.bytes[31] <= 127";
        let result = analyze_spec(spec);

        // Should be skipped as prose (contains "must be clear")
        assert!(result.is_err());
    }

    // REMOVED: test_remove_type_casts
    // The remove_type_casts function was removed because verus_syn handles type casts natively.
    // See lines 219-229 for explanation.

    #[test]
    fn test_chained_comparison_with_cast() {
        let spec = "0 <= r0 < (pow2(51) as int)";
        let metrics = analyze_spec(spec).unwrap();

        // Type cast should be removed, then chained comparison expanded
        println!("Chained comparison with cast: {:?}", metrics);
        assert!(metrics.halstead_length > 5);
    }

    // Note: Very complex chained comparisons with arithmetic operators in middle expressions
    // (e.g., "a <= b - c < d") are edge cases not handled by simple regex.
    // These are rare and can be skipped.

    #[test]
    fn test_function_calls_are_syntactic_only() {
        // This test demonstrates that Halstead metrics are computed SYNTACTICALLY,
        // NOT by recursively expanding function definitions.

        // Simple operands
        let m1 = analyze_spec("x == y").unwrap();
        println!(
            "\n'x == y' -> length={} (ops={}, operands={})",
            m1.halstead_length, m1.n1_total_operators, m1.n2_total_operands
        );
        // == : 1 operator
        // x, y : 2 operands
        // Total: 3
        assert_eq!(m1.halstead_length, 3);

        // Add function call f(x)
        let m2 = analyze_spec("f(x) == y").unwrap();
        println!(
            "'f(x) == y' -> length={} (ops={}, operands={})",
            m2.halstead_length, m2.n1_total_operators, m2.n2_total_operands
        );
        // call, == : 2 operators
        // f, x, y : 3 operands
        // Total: 5
        assert_eq!(m2.halstead_length, 5);

        // Two function calls: f(x) == g(y)
        let m3 = analyze_spec("f(x) == g(y)").unwrap();
        println!(
            "'f(x) == g(y)' -> length={} (ops={}, operands={})",
            m3.halstead_length, m3.n1_total_operators, m3.n2_total_operands
        );
        // call, ==, call : 3 operators
        // f, x, g, y : 4 operands
        // Total: 7
        assert_eq!(m3.halstead_length, 7);

        // Nested function call: f(g(x)) == y
        let m4 = analyze_spec("f(g(x)) == y").unwrap();
        println!(
            "'f(g(x)) == y' -> length={} (ops={}, operands={})",
            m4.halstead_length, m4.n1_total_operators, m4.n2_total_operands
        );
        // call (f), call (g), == : 3 operators
        // f, g, x, y : 4 operands
        // Total: 7
        assert_eq!(m4.halstead_length, 7);

        // Complex nesting: h(f(x), g(y))
        let m5 = analyze_spec("h(f(x), g(y))").unwrap();
        println!(
            "'h(f(x), g(y))' -> length={} (ops={}, operands={})",
            m5.halstead_length, m5.n1_total_operators, m5.n2_total_operands
        );
        // call (h), call (f), call (g) : 3 operators
        // h, f, x, g, y : 5 operands
        // Total: 8
        assert_eq!(m5.halstead_length, 8);

        println!("\n✅ Key: Function calls add 'call' operator + function name operand.");
        println!("   They do NOT recursively expand the function body!");
    }
}
