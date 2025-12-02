use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs::File;

#[derive(Debug, Deserialize)]
struct Atom {
    identifier: String,
    #[allow(dead_code)]
    statement_type: String,
    #[allow(dead_code)]
    deps: Vec<String>,
    body: String,
    display_name: String,
    #[allow(dead_code)]
    full_path: String,
    #[allow(dead_code)]
    relative_path: String,
    file_name: String,
    #[allow(dead_code)]
    parent_folder: String,
}

#[derive(Debug, Serialize)]
struct VerificationCategory {
    identifier: String,
    display_name: String,
    file_name: String,
    category: String,
    has_requires: bool,
    has_ensures: bool,
    has_proof_block: bool,
    has_assume_false: bool,
    is_trivially_verified: bool,
    requires_count: usize,
    ensures_count: usize,
}

fn count_spec_clauses(body: &str, keyword: &str) -> usize {
    let mut count = 0;
    let mut pos = 0;

    while let Some(found) = body[pos..].find(keyword) {
        pos += found + keyword.len();
        count += 1;
    }

    count
}

fn categorize_function(atom: &Atom) -> VerificationCategory {
    let body = &atom.body;

    // Check for various patterns
    let has_requires = body.contains("requires");
    let has_ensures = body.contains("ensures");
    let has_proof_block = body.contains("proof {");
    let has_assume_false = body.contains("assume(false)");

    // Count clauses (rough estimate)
    let requires_count = count_spec_clauses(body, "requires");
    let ensures_count = count_spec_clauses(body, "ensures");

    // Determine category
    let category = if has_proof_block {
        "verified_with_proof"
    } else if has_assume_false {
        "verified_with_assume_false"
    } else if has_ensures && !has_proof_block && !has_assume_false {
        "trivially_verified"
    } else if has_requires && !has_ensures {
        "only_requires"
    } else if has_ensures {
        "has_ensures_uncategorized"
    } else {
        "unspecified"
    };

    // Trivially verified = has ensures, no proof block, no assume(false)
    let is_trivially_verified = has_ensures && !has_proof_block && !has_assume_false;

    VerificationCategory {
        identifier: atom.identifier.clone(),
        display_name: atom.display_name.clone(),
        file_name: atom.file_name.clone(),
        category: category.to_string(),
        has_requires,
        has_ensures,
        has_proof_block,
        has_assume_false,
        is_trivially_verified,
        requires_count,
        ensures_count,
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!(
            "Usage: {} <input_atoms_json> <output_categorized_json>",
            args[0]
        );
        eprintln!();
        eprintln!("Categorizes functions by verification approach:");
        eprintln!("  1. verified_with_proof       - Has proof {{ }} blocks");
        eprintln!("  2. trivially_verified        - Has ensures, no proof, no assume(false)");
        eprintln!("  3. verified_with_assume_false - Has assume(false) (incomplete)");
        eprintln!("  4. only_requires             - Has requires but no ensures");
        eprintln!("  5. unspecified               - No specs");
        eprintln!();
        eprintln!("Example:");
        eprintln!("  {} \\", args[0]);
        eprintln!("    curve_dalek_atoms_24_nov.json \\");
        eprintln!("    verification_categories.json");
        std::process::exit(1);
    }

    let input_path = &args[1];
    let output_path = &args[2];

    println!("Loading atoms from {}...", input_path);
    let file = File::open(input_path)?;
    let atoms: Vec<Atom> = serde_json::from_reader(file)?;
    println!("  Loaded {} functions", atoms.len());

    println!("Categorizing verification approaches...");
    let categorized: Vec<VerificationCategory> = atoms.iter().map(categorize_function).collect();

    // Compute statistics
    let total = categorized.len();
    let with_proof = categorized.iter().filter(|c| c.has_proof_block).count();
    let trivially_verified = categorized
        .iter()
        .filter(|c| c.is_trivially_verified)
        .count();
    let with_assume_false = categorized.iter().filter(|c| c.has_assume_false).count();
    let with_specs = categorized
        .iter()
        .filter(|c| c.has_requires || c.has_ensures)
        .count();
    let verified_total = with_proof + trivially_verified;

    println!("Writing output to {}...", output_path);
    let output_file = File::create(output_path)?;
    serde_json::to_writer_pretty(output_file, &categorized)?;

    println!("✓ Done!");
    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!("VERIFICATION CATEGORIZATION SUMMARY");
    println!("═══════════════════════════════════════════════════════════════");
    println!();
    println!("Total functions: {}", total);
    println!();
    println!("Verification Categories:");
    println!(
        "  1. Verified with proof blocks:   {} ({:.1}%)",
        with_proof,
        (with_proof as f64 / total as f64) * 100.0
    );
    println!(
        "  2. Trivially verified (no proof): {} ({:.1}%)",
        trivially_verified,
        (trivially_verified as f64 / total as f64) * 100.0
    );
    println!(
        "  3. With assume(false):           {} ({:.1}%)",
        with_assume_false,
        (with_assume_false as f64 / total as f64) * 100.0
    );
    println!();
    println!(
        "  TOTAL VERIFIED: {} ({:.1}%)",
        verified_total,
        (verified_total as f64 / total as f64) * 100.0
    );
    println!();
    println!("Other categories:");
    println!(
        "  Functions with specs:            {} ({:.1}%)",
        with_specs,
        (with_specs as f64 / total as f64) * 100.0
    );
    println!(
        "  Unspecified:                     {} ({:.1}%)",
        total - with_specs,
        ((total - with_specs) as f64 / total as f64) * 100.0
    );
    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!();
    println!("Breakdown by category:");

    let mut category_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for cat in &categorized {
        *category_counts.entry(cat.category.clone()).or_insert(0) += 1;
    }

    let mut categories: Vec<_> = category_counts.iter().collect();
    categories.sort_by_key(|(_, count)| std::cmp::Reverse(**count));

    for (cat, count) in categories {
        println!(
            "  {}: {} ({:.1}%)",
            cat,
            count,
            (*count as f64 / total as f64) * 100.0
        );
    }

    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!();
    println!("Examples:");
    println!();

    // Show examples of trivially verified
    println!("Trivially verified functions (first 10):");
    let trivial_examples: Vec<_> = categorized
        .iter()
        .filter(|c| c.is_trivially_verified)
        .take(10)
        .collect();

    for ex in trivial_examples {
        println!("  • {} ({})", ex.display_name, ex.file_name);
        println!(
            "    requires: {}, ensures: {}",
            ex.requires_count, ex.ensures_count
        );
    }

    println!();
    println!("Functions with proof blocks (first 10):");
    let proof_examples: Vec<_> = categorized
        .iter()
        .filter(|c| c.has_proof_block && !c.display_name.starts_with("lemma_"))
        .take(10)
        .collect();

    for ex in proof_examples {
        println!("  • {} ({})", ex.display_name, ex.file_name);
    }

    Ok(())
}
