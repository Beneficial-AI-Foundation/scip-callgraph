//! Detailed validation: Show exactly what tokens are counted
//!
//! Usage: cargo run --bin validate_spec_halstead

use verus_metrics::spec_halstead::analyze_spec;

fn validate_spec(name: &str, spec: &str) {
    println!("\n{}", "=".repeat(80));
    println!("📋 Example: {}", name);
    println!("{}", "=".repeat(80));
    println!("Spec: {}\n", spec);
    
    match analyze_spec(spec) {
        Ok(metrics) => {
            println!("✅ Parse successful\n");
            
            // Show token counts
            println!("📊 Token Counts:");
            println!("  Unique Operators (n1):     {}", metrics.n1_unique_operators);
            println!("  Total Operators (N1):      {}", metrics.n1_total_operators);
            println!("  Unique Operands (n2):      {}", metrics.n2_unique_operands);
            println!("  Total Operands (N2):       {}", metrics.n2_total_operands);
            println!();
            
            // Show computed metrics
            println!("📈 Halstead Metrics:");
            println!("  Length (N1+N2):            {}", metrics.halstead_length);
            println!("  Vocabulary (n1+n2):        {}", metrics.vocabulary);
            println!("  Difficulty:                {:.3}", metrics.difficulty);
            println!("  Volume:                    {:.3}", metrics.volume);
            println!("  Effort:                    {:.3}", metrics.effort);
            println!();
            
            // Interpret complexity
            let complexity = if metrics.difficulty < 2.0 {
                "⭐ Very Simple"
            } else if metrics.difficulty < 5.0 {
                "⭐⭐ Simple"
            } else if metrics.difficulty < 10.0 {
                "⭐⭐⭐ Moderate"
            } else if metrics.difficulty < 20.0 {
                "⭐⭐⭐⭐ Complex"
            } else {
                "⭐⭐⭐⭐⭐ Very Complex"
            };
            
            println!("🎯 Interpretation: {}", complexity);
        }
        Err(e) => {
            println!("❌ Parse Error: {}", e);
        }
    }
}

fn main() {
    println!("\n🔬 Spec Halstead Validation - Detailed Token Analysis");
    
    // Example 1: Simplest possible spec
    validate_spec(
        "Single Comparison",
        "x < 10"
    );
    
    // Example 2: Two comparisons with AND
    validate_spec(
        "Conjunctive Bounds Check",
        "x < FIELD_MODULUS && y < FIELD_MODULUS"
    );
    
    // Example 3: Field access with comparison
    validate_spec(
        "Field Access Comparison",
        "result.0 < FIELD_MODULUS"
    );
    
    // Example 4: Complex arithmetic postcondition
    validate_spec(
        "Arithmetic Postcondition",
        "result.0 == (x.0 + y.0) % FIELD_MODULUS"
    );
    
    // Example 5: Array indexing
    validate_spec(
        "Array Indexing with Bounds",
        "i < arr.len() && arr[i] > 0"
    );
    
    // Example 6: Multiple field accesses
    validate_spec(
        "Byte Equality Check",
        "self.bytes[0] == other.bytes[0] && self.bytes[1] == other.bytes[1]"
    );
    
    // Example 7: Verus forall
    validate_spec(
        "Verus Forall Quantifier",
        "forall|i: usize| i < len ==> arr[i] > 0"
    );
    
    // Example 8: Complex nested expression
    validate_spec(
        "Nested Arithmetic",
        "(a + b) * (c - d) == result"
    );
    
    // Example 9: Method calls
    validate_spec(
        "Method Call Chain",
        "self.is_valid() && other.is_valid()"
    );
    
    // Example 10: Negation and casting
    validate_spec(
        "Negation with Cast",
        "!flag && (x as u64) > MAX_VALUE"
    );
    
    println!("\n{}", "=".repeat(80));
    println!("✅ Validation Complete");
    println!("{}", "=".repeat(80));
    
    println!("\n💡 Observations:");
    println!("  • Length increases with number of tokens");
    println!("  • Difficulty increases with operator diversity");
    println!("  • Repeated operands reduce difficulty (more reuse)");
    println!("  • Field access (.) and indexing ([]) count as operators");
    println!("  • Each unique variable/constant counts once in n2");
}

