use csv::Reader;
use serde::Deserialize;
use std::collections::HashSet;
use std::error::Error;
use std::fs::File;

#[derive(Debug, Deserialize)]
struct CsvRow {
    function: String,
    module: String,
    link: String,
    #[allow(dead_code)]
    has_spec: String,
    has_proof: String,
    trivial_proof: String,
}

#[derive(Debug, Deserialize)]
struct Atom {
    identifier: String,
    display_name: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} <csv_with_trivial> <atoms_json>", args[0]);
        eprintln!();
        eprintln!("Finds verified functions with non-trivial proofs (trivial_proof=no)");
        eprintln!("that are MISSING from the atoms JSON.");
        eprintln!();
        eprintln!("Example:");
        eprintln!("  {} \\", args[0]);
        eprintln!("    curve25519_functions_with_trivial.csv \\");
        eprintln!("    curve_dalek_atoms_24_nov.json");
        std::process::exit(1);
    }

    let csv_path = &args[1];
    let atoms_path = &args[2];

    // Load atoms JSON
    println!("Loading atoms from {}...", atoms_path);
    let file = File::open(atoms_path)?;
    let atoms: Vec<Atom> = serde_json::from_reader(file)?;
    println!("  Loaded {} atoms", atoms.len());

    // Build set of function names in atoms
    let mut atom_names: HashSet<String> = HashSet::new();
    for atom in &atoms {
        atom_names.insert(atom.display_name.clone());

        // Also extract the function name from identifier
        // Format: "4.1.3 module/path/Type/function"
        if let Some(path_part) = atom.identifier.split_whitespace().nth(1) {
            if let Some(function) = path_part.split('/').next_back() {
                atom_names.insert(function.to_string());
            }
        }
    }

    println!(
        "  Built index of {} unique function names",
        atom_names.len()
    );

    // Load CSV and find missing functions
    println!("Reading CSV from {}...", csv_path);
    let mut reader = Reader::from_path(csv_path)?;

    let mut verified_with_proofs = Vec::new();
    let mut missing = Vec::new();

    for result in reader.deserialize() {
        let row: CsvRow = result?;

        // Filter for: has_proof=yes AND trivial_proof=no (non-trivial proofs)
        if row.has_proof == "yes" && row.trivial_proof == "no" {
            verified_with_proofs.push(row.function.clone());

            // Check if in atoms JSON
            // Try exact match first
            let found = if atom_names.contains(&row.function) {
                true
            } else {
                // Try stripping type prefix (e.g., "FieldElement51::add" -> "add")
                if let Some(pos) = row.function.rfind("::") {
                    let stripped = &row.function[pos + 2..];
                    atom_names.contains(stripped)
                } else {
                    false
                }
            };

            if !found {
                missing.push((row.function, row.module, row.link));
            }
        }
    }

    println!("✓ Done!");
    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!("MISSING VERIFIED FUNCTIONS WITH NON-TRIVIAL PROOFS");
    println!("═══════════════════════════════════════════════════════════════");
    println!();
    println!(
        "Total verified functions with proofs (trivial_proof=no): {}",
        verified_with_proofs.len()
    );
    println!(
        "Found in atoms JSON: {}",
        verified_with_proofs.len() - missing.len()
    );
    println!("MISSING from atoms JSON: {}", missing.len());
    println!();

    if missing.is_empty() {
        println!("✅ All verified functions with proofs are present in atoms JSON!");
    } else {
        println!(
            "⚠️  The following {} functions need to be regenerated:",
            missing.len()
        );
        println!();

        for (i, (function, module, link)) in missing.iter().enumerate() {
            println!("{}. {}", i + 1, function);
            println!("   Module: {}", module);
            println!("   Link: {}", link);
            println!();
        }

        println!("═══════════════════════════════════════════════════════════════");
        println!();
        println!("To regenerate atoms JSON with these functions:");
        println!();
        println!("  cargo run --release --bin write_atoms \\");
        println!("    curve_dalek_index_scip_26_nov.json \\");
        println!("    curve_dalek_atoms_26_nov_NEW.json");
        println!();
        println!("Then rerun your analysis with the new atoms JSON.");
    }

    Ok(())
}
