        use serde::{Deserialize, Serialize};                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                
use std::collections::HashMap;
use std::fs;
use verus_metrics::spec_halstead::analyze_spec;

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
    requires_count: usize,
    requires_lengths: Vec<usize>,
    requires_specs: Vec<SpecHalsteadMetrics>,
    ensures_count: usize,
    ensures_lengths: Vec<usize>,
    ensures_specs: Vec<SpecHalsteadMetrics>,
    decreases_count: usize,
    decreases_specs: Vec<SpecHalsteadMetrics>,  // Halstead metrics for each decreases expression
    body_length: usize,
    operators: HashMap<String, usize>,
}

/// Clean signature: remove comments, triggers, newlines, and ghost operators
fn clean_signature(text: &str) -> String {
    // Step 1: Remove comments
    let without_comments = remove_comments(text);
    
    // Step 2: Remove triggers
    let without_triggers = remove_triggers(&without_comments);
    
    // Step 3: Remove newlines
    let without_newlines = without_triggers.replace('\n', " ");
    
    // Step 4: Remove @ ghost operator (Verus-specific)
    without_newlines.replace('@', "")
}

/// Find the function body start by looking for { at depth 0
/// (not inside parentheses/brackets/braces from specs)
/// Skip { blocks that end with comma (spec blocks like "==> { ... },")
fn find_function_body_start(body: &str) -> usize {
    let chars: Vec<char> = body.chars().collect();
    let mut depth: i32 = 0;
    let mut i = 0;
    
    while i < chars.len() {
        match chars[i] {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            '{' => {
                if depth == 0 {
                    // Found { at depth 0 - check if it's a spec block
                    // Spec blocks end with },  (comma after })
                    // Function bodies don't have comma after }
                    
                    // Find the matching }
                    let mut brace_depth = 1;
                    let mut j = i + 1;
                    while j < chars.len() && brace_depth > 0 {
                        match chars[j] {
                            '{' => brace_depth += 1,
                            '}' => brace_depth -= 1,
                            _ => {}
                        }
                        j += 1;
                    }
                    
                    // j is now after the matching }
                    // Skip whitespace and check for comma
                    let mut k = j;
                    while k < chars.len() && chars[k].is_whitespace() {
                        k += 1;
                    }
                    
                    if k < chars.len() && chars[k] == ',' {
                        // This { } block ends with comma - it's a spec block
                        // Continue searching from after the comma
                        i = k + 1;
                        continue;
                    } else {
                        // No comma after } - this is the function body
                        return i;
                    }
                }
                depth += 1;
            }
            '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
        i += 1;
    }
    
    body.len()
}

fn extract_requires(body: &str) -> (usize, Vec<usize>, String) {
    // Check if body has a function signature
    // Include Verus function modifiers: proof, exec, spec, tracked
    let trimmed = body.trim_start();
    let has_fn_signature = trimmed.starts_with("fn ")
        || trimmed.starts_with("pub fn ")
        || trimmed.starts_with("pub(crate) fn ")
        || trimmed.starts_with("pub(super) fn ")
        || trimmed.starts_with("proof fn ")
        || trimmed.starts_with("exec fn ")
        || trimmed.starts_with("spec fn ")
        || trimmed.starts_with("tracked fn ")
        || trimmed.starts_with("pub proof fn ")
        || trimmed.starts_with("pub exec fn ")
        || trimmed.starts_with("pub spec fn ")
        || trimmed.starts_with("pub tracked fn ")
        || trimmed.starts_with("pub(crate) proof fn ")
        || trimmed.starts_with("pub(crate) exec fn ")
        || trimmed.starts_with("pub(crate) spec fn ")
        || trimmed.starts_with("pub(crate) tracked fn ");
    
    if !has_fn_signature {
        // This body doesn't have a function signature, only body code
        // Don't extract any specs from it
        return (0, Vec::new(), String::new());
    }
    
    // Find the function body start ({ at depth 0)
    let function_body_start = find_function_body_start(body);
    
    // Only work with the signature (before function body)
    let signature = &body[..function_body_start];
    
    // Clean the signature: remove comments, triggers, newlines
    let cleaned = clean_signature(signature);
    
    // Find "requires" in the cleaned signature
    let start_opt = cleaned.find("requires");
    if start_opt.is_none() {
        return (0, Vec::new(), String::new());
    }
    let start = start_opt.unwrap() + "requires".len();
    
    // Find end: either "ensures", "returns", or end of cleaned signature
    let after_requires = &cleaned[start..];
    let mut end = start + after_requires.len();
    
    if let Some(pos) = after_requires.find("ensures") {
        end = start + pos;
    }
    if let Some(pos) = after_requires.find("returns") {
        if start + pos < end {
            end = start + pos;
        }
    }
    
    let requires_block = &cleaned[start..end];
    
    if requires_block.trim().is_empty() {
        return (0, Vec::new(), String::new());
    }
    
    let conditions = split_conditions(requires_block);
    let count = conditions.len();
    let lengths: Vec<usize> = conditions.iter().map(|c| c.trim().len()).collect();
    
    (count, lengths, requires_block.to_string())
}

fn extract_ensures(body: &str) -> (usize, Vec<usize>, String) {
    // Check if body has a function signature
    // Include Verus function modifiers: proof, exec, spec, tracked
    let trimmed = body.trim_start();
    let has_fn_signature = trimmed.starts_with("fn ")
        || trimmed.starts_with("pub fn ")
        || trimmed.starts_with("pub(crate) fn ")
        || trimmed.starts_with("pub(super) fn ")
        || trimmed.starts_with("proof fn ")
        || trimmed.starts_with("exec fn ")
        || trimmed.starts_with("spec fn ")
        || trimmed.starts_with("tracked fn ")
        || trimmed.starts_with("pub proof fn ")
        || trimmed.starts_with("pub exec fn ")
        || trimmed.starts_with("pub spec fn ")
        || trimmed.starts_with("pub tracked fn ")
        || trimmed.starts_with("pub(crate) proof fn ")
        || trimmed.starts_with("pub(crate) exec fn ")
        || trimmed.starts_with("pub(crate) spec fn ")
        || trimmed.starts_with("pub(crate) tracked fn ");
    
    if !has_fn_signature {
        // This body doesn't have a function signature, only body code
        // Don't extract any specs from it
        return (0, Vec::new(), String::new());
    }
    
    // Find the function body start ({ at depth 0)
    let function_body_start = find_function_body_start(body);
    
    // Only work with the signature (before function body)
    let signature = &body[..function_body_start];
    
    // Clean the signature: remove comments, triggers, newlines
    let cleaned = clean_signature(signature);
    
    // Find "ensures" in the cleaned signature
    let start_opt = cleaned.find("ensures");
    if start_opt.is_none() {
        return (0, Vec::new(), String::new());
    }
    let start = start_opt.unwrap() + "ensures".len();
    
    // Find end: either "returns" or end of cleaned signature
    let after_ensures = &cleaned[start..];
    let mut end = start + after_ensures.len();
    
    if let Some(pos) = after_ensures.find("returns") {
        end = start + pos;
    }
    
    let ensures_block = &cleaned[start..end];
    
    if ensures_block.trim().is_empty() {
        return (0, Vec::new(), String::new());
    }
    
    let conditions = split_conditions(ensures_block);
    let count = conditions.len();
    let lengths: Vec<usize> = conditions.iter().map(|c| c.trim().len()).collect();
    
    (count, lengths, ensures_block.to_string())
}

fn extract_decreases(body: &str) -> (usize, Vec<String>) {
    // Check if body has a function signature (same check as requires/ensures)
    let trimmed = body.trim_start();
    let has_fn_signature = trimmed.starts_with("fn ")
        || trimmed.starts_with("pub fn ")
        || trimmed.starts_with("pub(crate) fn ")
        || trimmed.starts_with("pub(super) fn ")
        || trimmed.starts_with("proof fn ")
        || trimmed.starts_with("exec fn ")
        || trimmed.starts_with("spec fn ")
        || trimmed.starts_with("tracked fn ")
        || trimmed.starts_with("pub proof fn ")
        || trimmed.starts_with("pub exec fn ")
        || trimmed.starts_with("pub spec fn ")
        || trimmed.starts_with("pub tracked fn ")
        || trimmed.starts_with("pub(crate) proof fn ")
        || trimmed.starts_with("pub(crate) exec fn ")
        || trimmed.starts_with("pub(crate) spec fn ")
        || trimmed.starts_with("pub(crate) tracked fn ");
    
    if !has_fn_signature {
        return (0, Vec::new());
    }
    
    // Find the function body start
    let function_body_start = find_function_body_start(body);
    
    // Only work with the signature (before function body)
    let signature = &body[..function_body_start];
    
    // Clean the signature: remove comments, triggers, newlines
    let cleaned = clean_signature(signature);
    
    // Find "decreases" in the cleaned signature
    let start_opt = cleaned.find("decreases");
    if start_opt.is_none() {
        return (0, Vec::new());
    }
    let start = start_opt.unwrap() + "decreases".len();
    
    // Find end: look for "requires", "ensures", "returns", or end of cleaned signature
    let after_decreases = &cleaned[start..];
    let mut end = start + after_decreases.len();
    
    // Check for other spec keywords
    for keyword in ["requires", "ensures", "returns"] {
        if let Some(pos) = after_decreases.find(keyword) {
            if start + pos < end {
                end = start + pos;
            }
        }
    }
    
    let decreases_block = &cleaned[start..end].trim();
    
    if decreases_block.is_empty() {
        return (0, Vec::new());
    }
    
    // Split by comma to get individual expressions
    // Use the same split_conditions logic but simpler since decreases is usually simpler
    let expressions = split_conditions(decreases_block);
    let count = expressions.len();
    
    (count, expressions)
}

/// Remove comments from a string
fn remove_comments(text: &str) -> String {
    let mut result = String::new();
    let mut chars = text.chars().peekable();
    
    #[allow(clippy::while_let_on_iterator)]
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                // Preserve string literals as-is
                result.push(c);
                #[allow(clippy::while_let_on_iterator)]
                while let Some(ch) = chars.next() {
                    result.push(ch);
                    if ch == '\\' {
                        if let Some(escaped) = chars.next() {
                            result.push(escaped);
                        }
                    } else if ch == '"' {
                        break;
                    }
                }
            }
            '/' => {
                let next = chars.peek().copied();
                if next == Some('/') {
                    // Line comment - skip to end of line
                    chars.next(); // consume second /
                    #[allow(clippy::while_let_on_iterator)]
                    while let Some(ch) = chars.next() {
                        if ch == '\n' {
                            result.push('\n'); // Keep newline
                            break;
                        }
                    }
                } else if next == Some('*') {
                    // Block comment - skip to */
                    chars.next(); // consume *
                    let mut prev = ' ';
                    #[allow(clippy::while_let_on_iterator)]
                    while let Some(ch) = chars.next() {
                        if prev == '*' && ch == '/' {
                            break;
                        }
                        prev = ch;
                    }
                } else {
                    result.push(c);
                }
            }
            _ => {
                result.push(c);
            }
        }
    }
    
    result
}

/// Remove trigger annotations like #![trigger ...] from text
/// Handles nested brackets like #![trigger old(inputs)[i]]
fn remove_triggers(text: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    
    while i < chars.len() {
        // Check for #![trigger
        if i + 10 < chars.len() {
            let slice: String = chars[i..i+10].iter().collect();
            if slice == "#![trigger" {
                // Found trigger annotation - skip until we find the closing ]
                i += 10;
                let mut bracket_depth = 1; // We're inside the #![
                while i < chars.len() && bracket_depth > 0 {
                    match chars[i] {
                        '[' => bracket_depth += 1,
                        ']' => bracket_depth -= 1,
                        _ => {}
                    }
                    i += 1;
                }
                continue;
            }
        }
        result.push(chars[i]);
        i += 1;
    }
    
    result
}

fn split_conditions(block: &str) -> Vec<String> {
    // Step 1: Remove comments (including comment-only lines)
    let without_comments = remove_comments(block);
    
    // Step 2: Remove trigger annotations
    let without_triggers = remove_triggers(&without_comments);
    
    // Step 3: Remove newlines to handle multi-line conditions
    let single_line = without_triggers.replace('\n', " ");
    
    // Step 4: Split by commas at depth 0
    let mut conditions = Vec::new();
    let mut current = String::new();
    let mut depth = 0;
    let mut in_string = false;
    let mut escape_next = false;
    
    let chars: Vec<char> = single_line.chars().collect();
    let mut i = 0;
    
    while i < chars.len() {
        let c = chars[i];
        
        if escape_next {
            current.push(c);
            escape_next = false;
            i += 1;
            continue;
        }
        
        if c == '\\' && in_string {
            current.push(c);
            escape_next = true;
            i += 1;
            continue;
        }
        
        if c == '"' {
            in_string = !in_string;
            current.push(c);
            i += 1;
            continue;
        }
        
        if !in_string {
            match c {
                '(' | '[' | '{' => {
                    depth += 1;
                    current.push(c);
                }
                ')' | ']' | '}' => {
                    depth -= 1;
                    current.push(c);
                }
                ',' if depth == 0 => {
                    let trimmed = current.trim();
                    if !trimmed.is_empty() {
                        conditions.push(trimmed.to_string());
                    }
                    current.clear();
                }
                _ => {
                    current.push(c);
                }
            }
        } else {
            current.push(c);
        }
        
        i += 1;
    }
    
    let trimmed = current.trim();
    if !trimmed.is_empty() {
        conditions.push(trimmed.to_string());
    }
    
    conditions
}

fn remove_strings_and_comments(code: &str) -> String {
    let mut result = String::new();
    let mut chars = code.chars().peekable();
    
    #[allow(clippy::while_let_on_iterator)]
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                // Skip string literals
                #[allow(clippy::while_let_on_iterator)]
                while let Some(ch) = chars.next() {
                    if ch == '\\' {
                        chars.next(); // Skip escaped character
                    } else if ch == '"' {
                        break;
                    }
                }
            }
            '/' => {
                let next = chars.peek().copied();
                if next == Some('/') {
                    // Line comment - skip to end of line
                    chars.next(); // consume second /
                    #[allow(clippy::while_let_on_iterator)]
                    while let Some(ch) = chars.next() {
                        if ch == '\n' {
                            result.push('\n');
                            break;
                        }
                    }
                } else if next == Some('*') {
                    // Block comment
                    chars.next(); // consume *
                    let mut prev = ' ';
                    #[allow(clippy::while_let_on_iterator)]
                    while let Some(ch) = chars.next() {
                        if prev == '*' && ch == '/' {
                            break;
                        }
                        prev = ch;
                    }
                } else {
                    result.push(c);
                }
            }
            _ => {
                result.push(c);
            }
        }
    }
    
    result
}

fn count_operators(code: &str) -> HashMap<String, usize> {
    let cleaned = remove_strings_and_comments(code);
    
    let operators = vec![
        // Process longer operators first to avoid miscounting
        "<<", ">>", "<=", ">=", "==", "!=", "&&", "||",
        "+", "-", "*", "/", "%",
        "&", "|", "^",
        "<", ">",
        "!",
    ];
    
    let mut counts: HashMap<String, usize> = HashMap::new();
    
    for op in operators {
        let count = cleaned.matches(op).count();
        if count > 0 {
            counts.insert(op.to_string(), count);
        }
    }
    
    counts
}

fn compute_function_metrics(atom: &Atom) -> FunctionMetrics {
    let body = &atom.body;
    
    // Extract requires
    let (_requires_count, _requires_lengths, requires_text) = extract_requires(body);
    let requires_conditions = if !requires_text.is_empty() {
        split_conditions(&requires_text)
    } else {
        Vec::new()
    };
    
    // Extract ensures
    let (_ensures_count, _ensures_lengths, ensures_text) = extract_ensures(body);
    let ensures_conditions = if !ensures_text.is_empty() {
        split_conditions(&ensures_text)
    } else {
        Vec::new()
    };
    
    // Filter out prose conditions (comments, documentation, etc.)
    let requires_conditions_filtered: Vec<String> = requires_conditions
        .into_iter()
        .filter(|cond| !verus_metrics::spec_halstead::is_prose(cond))
        .collect();
    
    let ensures_conditions_filtered: Vec<String> = ensures_conditions
        .into_iter()
        .filter(|cond| !verus_metrics::spec_halstead::is_prose(cond))
        .collect();
    
    // Update counts to reflect filtered conditions
    let filtered_requires_count = requires_conditions_filtered.len();
    let filtered_requires_lengths: Vec<usize> = requires_conditions_filtered.iter()
        .map(|c| c.trim().len())
        .collect();
    
    let filtered_ensures_count = ensures_conditions_filtered.len();
    let filtered_ensures_lengths: Vec<usize> = ensures_conditions_filtered.iter()
        .map(|c| c.trim().len())
        .collect();
    
    // Compute Halstead metrics for each requires condition
    let requires_specs: Vec<SpecHalsteadMetrics> = requires_conditions_filtered
        .iter()
        .map(|cond| {
            match analyze_spec(cond) {
                Ok(metrics) => SpecHalsteadMetrics {
                    text: cond.clone(),
                    halstead_length: Some(metrics.halstead_length),
                    halstead_difficulty: Some(metrics.difficulty),
                    halstead_effort: Some(metrics.effort),
                    halstead_vocabulary: Some(metrics.vocabulary),
                    halstead_volume: Some(metrics.volume),
                    unique_operators: Some(metrics.n1_unique_operators),
                    total_operators: Some(metrics.n1_total_operators),
                    unique_operands: Some(metrics.n2_unique_operands),
                    total_operands: Some(metrics.n2_total_operands),
                    parse_error: None,
                },
                Err(e) => SpecHalsteadMetrics {
                    text: cond.clone(),
                    halstead_length: None,
                    halstead_difficulty: None,
                    halstead_effort: None,
                    halstead_vocabulary: None,
                    halstead_volume: None,
                    unique_operators: None,
                    total_operators: None,
                    unique_operands: None,
                    total_operands: None,
                    parse_error: Some(e.to_string()),
                },
            }
        })
        .collect();
    
    // Compute Halstead metrics for each ensures condition
    let ensures_specs: Vec<SpecHalsteadMetrics> = ensures_conditions_filtered
        .iter()
        .map(|cond| {
            match analyze_spec(cond) {
                Ok(metrics) => SpecHalsteadMetrics {
                    text: cond.clone(),
                    halstead_length: Some(metrics.halstead_length),
                    halstead_difficulty: Some(metrics.difficulty),
                    halstead_effort: Some(metrics.effort),
                    halstead_vocabulary: Some(metrics.vocabulary),
                    halstead_volume: Some(metrics.volume),
                    unique_operators: Some(metrics.n1_unique_operators),
                    total_operators: Some(metrics.n1_total_operators),
                    unique_operands: Some(metrics.n2_unique_operands),
                    total_operands: Some(metrics.n2_total_operands),
                    parse_error: None,
                },
                Err(e) => SpecHalsteadMetrics {
                    text: cond.clone(),
                    halstead_length: None,
                    halstead_difficulty: None,
                    halstead_effort: None,
                    halstead_vocabulary: None,
                    halstead_volume: None,
                    unique_operators: None,
                    total_operators: None,
                    unique_operands: None,
                    total_operands: None,
                    parse_error: Some(e.to_string()),
                },
            }
        })
        .collect();
    
    // Extract decreases clause (for termination proofs)
    let (decreases_count, decreases_expressions) = extract_decreases(body);
    
    // Compute Halstead metrics for each decreases expression
    let decreases_specs: Vec<SpecHalsteadMetrics> = decreases_expressions
        .iter()
        .map(|expr| {
            match analyze_spec(expr) {
                Ok(metrics) => SpecHalsteadMetrics {
                    text: expr.clone(),
                    halstead_length: Some(metrics.halstead_length),
                    halstead_difficulty: Some(metrics.difficulty),
                    halstead_effort: Some(metrics.effort),
                    halstead_vocabulary: Some(metrics.vocabulary),
                    halstead_volume: Some(metrics.volume),
                    unique_operators: Some(metrics.n1_unique_operators),
                    total_operators: Some(metrics.n1_total_operators),
                    unique_operands: Some(metrics.n2_unique_operands),
                    total_operands: Some(metrics.n2_total_operands),
                    parse_error: None,
                },
                Err(e) => SpecHalsteadMetrics {
                    text: expr.clone(),
                    halstead_length: None,
                    halstead_difficulty: None,
                    halstead_effort: None,
                    halstead_vocabulary: None,
                    halstead_volume: None,
                    unique_operators: None,
                    total_operators: None,
                    unique_operands: None,
                    total_operands: None,
                    parse_error: Some(e.to_string()),
                },
            }
        })
        .collect();
    
    // Calculate body length (excluding requires and ensures)
    let total_spec_length = requires_text.len() + ensures_text.len();
    let body_length = body.len().saturating_sub(total_spec_length);
    
    // Remove requires and ensures from body for operator counting
    let mut body_without_specs = body.clone();
    if !requires_text.is_empty() {
        body_without_specs = body_without_specs.replace(&requires_text, "");
    }
    if !ensures_text.is_empty() {
        body_without_specs = body_without_specs.replace(&ensures_text, "");
    }
    
    // Count operators in body (excluding specs)
    let operators = count_operators(&body_without_specs);
    
    FunctionMetrics {
        requires_count: filtered_requires_count,
        requires_lengths: filtered_requires_lengths,
        requires_specs,
        ensures_count: filtered_ensures_count,
        ensures_lengths: filtered_ensures_lengths,
        ensures_specs,
        decreases_count,
        decreases_specs,
        body_length,
        operators,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} <input_atoms_json> <output_metrics_json>", args[0]);
        eprintln!("\nExample:");
        eprintln!("  {} curve_dalek_atoms.json curve_dalek_atoms_with_metrics.json", args[0]);
        std::process::exit(1);
    }
    
    let input_path = &args[1];
    let output_path = &args[2];
    
    println!("Loading atoms from {}...", input_path);
    let content = fs::read_to_string(input_path)
        .unwrap_or_else(|e| {
            eprintln!("Failed to read input file: {}", e);
            std::process::exit(1);
        });
    
    let atoms: Vec<Atom> = serde_json::from_str(&content)
        .unwrap_or_else(|e| {
            eprintln!("Failed to parse input JSON: {}", e);
            std::process::exit(1);
        });
    
    println!("  Loaded {} functions", atoms.len());
    
    println!("Computing metrics...");
    let atoms_with_metrics: Vec<AtomWithMetrics> = atoms
        .iter()
        .map(|atom| {
            let metrics = compute_function_metrics(atom);
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
    let output_json = serde_json::to_string_pretty(&atoms_with_metrics)
        .unwrap_or_else(|e| {
            eprintln!("Failed to serialize output: {}", e);
            std::process::exit(1);
        });
    
    fs::write(output_path, output_json)
        .unwrap_or_else(|e| {
            eprintln!("Failed to write output file: {}", e);
            std::process::exit(1);
        });
    
    println!("✓ Done!");
    
    // Print summary statistics
    let with_requires: usize = atoms_with_metrics.iter().filter(|a| a.metrics.requires_count > 0).count();
    let with_ensures: usize = atoms_with_metrics.iter().filter(|a| a.metrics.ensures_count > 0).count();
    let with_decreases: usize = atoms_with_metrics.iter().filter(|a| a.metrics.decreases_count > 0).count();
    
    // Count parse errors
    let requires_parse_errors: usize = atoms_with_metrics.iter()
        .flat_map(|a| &a.metrics.requires_specs)
        .filter(|spec| spec.parse_error.is_some())
        .count();
    let ensures_parse_errors: usize = atoms_with_metrics.iter()
        .flat_map(|a| &a.metrics.ensures_specs)
        .filter(|spec| spec.parse_error.is_some())
        .count();
    let decreases_parse_errors: usize = atoms_with_metrics.iter()
        .flat_map(|a| &a.metrics.decreases_specs)
        .filter(|spec| spec.parse_error.is_some())
        .count();
    
    println!("\nSummary:");
    println!("  Total functions: {}", atoms_with_metrics.len());
    println!("  With requires: {}", with_requires);
    println!("  With ensures: {}", with_ensures);
    println!("  With decreases: {}", with_decreases);
    println!("  Spec Halstead parse errors: {} requires, {} ensures, {} decreases", 
             requires_parse_errors, ensures_parse_errors, decreases_parse_errors);
    
    if let Some(example) = atoms_with_metrics.iter().find(|a| a.metrics.requires_count > 0 || a.metrics.ensures_count > 0) {
        println!("\nExample function with specs:");
        println!("  Name: {}", example.display_name);
        println!("  Requires: {}", example.metrics.requires_count);
        println!("  Ensures: {}", example.metrics.ensures_count);
        println!("  Body length: {}", example.metrics.body_length);
        println!("  Operators: {}", example.metrics.operators.len());
        
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

