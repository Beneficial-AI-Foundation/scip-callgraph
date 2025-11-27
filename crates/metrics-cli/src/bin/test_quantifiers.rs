/// Test Verus quantifier handling
///
/// Usage: cargo run --bin test_quantifiers

use verus_metrics::spec_halstead::analyze_spec;

fn test_spec(name: &str, spec: &str, expected_notes: &str) {
    println!("\n{}", "=".repeat(80));
    println!("📋 Test: {}", name);
    println!("{}", "=".repeat(80));
    println!("Spec: {}\n", spec);
    
    match analyze_spec(spec) {
        Ok(metrics) => {
            println!("✅ Parse successful\n");
            
            println!("📊 Metrics:");
            println!("  Length:      {}", metrics.halstead_length);
            println!("  Difficulty:  {:.3}", metrics.difficulty);
            println!("  Effort:      {:.3}", metrics.effort);
            println!("  Vocabulary:  {}", metrics.vocabulary);
            println!();
            
            println!("🔍 Token Details:");
            println!("  Unique operators (n1):    {}", metrics.n1_unique_operators);
            println!("  Total operators (N1):     {}", metrics.n1_total_operators);
            println!("  Unique operands (n2):     {}", metrics.n2_unique_operands);
            println!("  Total operands (N2):      {}", metrics.n2_total_operands);
            println!();
            
            println!("💡 Notes: {}", expected_notes);
        }
        Err(e) => {
            println!("❌ Parse Error: {}", e);
        }
    }
}

fn main() {
    println!("\n🔬 Verus Quantifier Testing");
    
    // Test 1: Simple forall
    test_spec(
        "Simple Forall",
        "forall|i: usize| i < len",
        "Closure with simple comparison"
    );
    
    // Test 2: Forall with implication
    test_spec(
        "Forall with Implication",
        "forall|i: usize| i < len ==> arr[i] > 0",
        "Implication (==>) converted to || in preprocessing"
    );
    
    // Test 3: Forall with complex body
    test_spec(
        "Forall with Complex Body",
        "forall|i: usize| 0 <= i && i < len ==> arr[i] == old(arr)[i]",
        "Multiple conditions, implication, and old() reference"
    );
    
    // Test 4: Nested forall
    test_spec(
        "Nested Forall",
        "forall|i: usize| i < n ==> forall|j: usize| j < m ==> matrix[i][j] == 0",
        "Two levels of quantification with implications"
    );
    
    // Test 5: Exists quantifier
    test_spec(
        "Exists Quantifier",
        "exists|i: usize| i < len && arr[i] == target",
        "Existential quantifier with conjunction"
    );
    
    // Test 6: Mixed quantifiers
    test_spec(
        "Mixed Quantifiers",
        "forall|x: usize| x < len ==> exists|y: usize| y < len && arr[x] == arr[y]",
        "Forall contains exists - complex quantifier nesting"
    );
    
    // Test 7: Forall with multiple variables
    test_spec(
        "Multiple Variables",
        "forall|i: usize, j: usize| i < j && j < len ==> arr[i] <= arr[j]",
        "Multiple bound variables in single quantifier"
    );
    
    // Test 8: Complex arithmetic in quantifier
    test_spec(
        "Arithmetic in Quantifier",
        "forall|i: usize| i < len ==> arr[i] == (i * SCALE + OFFSET) % MODULUS",
        "Complex arithmetic expression in quantifier body"
    );
    
    // Test 9: Quantifier with field access
    test_spec(
        "Field Access in Quantifier",
        "forall|i: usize| i < self.len() ==> self.data[i].is_valid()",
        "Field access and method calls within quantifier"
    );
    
    // Test 10: Real-world example (sorted array)
    test_spec(
        "Sorted Array Spec",
        "forall|i: usize, j: usize| 0 <= i && i < j && j < len ==> arr[i] <= arr[j]",
        "Typical sortedness specification"
    );
    
    println!("\n{}", "=".repeat(80));
    println!("✅ Quantifier Testing Complete");
    println!("{}", "=".repeat(80));
    
    println!("\n🔍 Analysis:");
    println!("  • forall|x| ... converted to |x| ... (closure syntax)");
    println!("  • exists|x| ... converted to |x| ... (closure syntax)");
    println!("  • ==> converted to || (logical OR approximation)");
    println!("  • old() treated as regular function call");
    println!("  • Nesting preserved through AST structure");
    println!();
    println!("📊 Observations:");
    println!("  • Quantifier body complexity dominates metrics");
    println!("  • Nested quantifiers significantly increase difficulty");
    println!("  • Implications (==>) add to operator count");
    println!("  • Multiple bound variables increase vocabulary");
}

