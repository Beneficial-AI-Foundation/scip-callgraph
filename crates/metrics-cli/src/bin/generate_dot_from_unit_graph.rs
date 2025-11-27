use serde_json::Value;
use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Read the JSON file
    let json_content = fs::read_to_string("unit_graph.json")?;
    let unit_graph: Value = serde_json::from_str(&json_content)?;

    // Start DOT file
    let mut dot_content = String::from("digraph dependencies {\n");
    dot_content.push_str("  node [shape=box];\n");

    // Process nodes
    if let Some(units) = unit_graph.get("units").and_then(|u| u.as_array()) {
        for unit in units {
            if let Some(name) = unit.get("name").and_then(|n| n.as_str()) {
                let escaped_name = name.replace("\"", "\\\"");
                dot_content.push_str(&format!(
                    "  \"{escaped_name}\" [label=\"{escaped_name}\"];\n"
                ));
            }
        }

        for unit in units {
            if let (Some(name), Some(deps)) = (
                unit.get("target")
                    .and_then(|profile| profile.get("name").and_then(|n| n.as_str())),
                unit.get("dependencies").and_then(|d| d.as_array()),
            ) {
                let source_name = name.replace("\"", "\\\"");
                println!("Processing unit: {source_name}");
                for dep in deps {
                    if let Some(dep_name) = dep.get("extern_crate_name").and_then(|n| n.as_str()) {
                        let escaped_target = dep_name.replace("\"", "\\\"");
                        dot_content
                            .push_str(&format!("  \"{source_name}\" -> \"{escaped_target}\";\n"));
                    }
                }
            }
        }
    } else {
        eprintln!("Error: 'units' field is missing or not an array in unit_graph.json");
    }

    // Close DOT file
    dot_content.push_str("}\n");

    // Write to file
    fs::write("dependency-graph.dot", dot_content)?;
    println!("DOT file generated: dependency-graph.dot");

    // Generate SVG from DOT file
    let svg_output = std::process::Command::new("dot")
        .args([
            "-Tsvg",
            "dependency-graph.dot",
            "-o",
            "dependency-graph.svg",
        ])
        .output()?;

    if !svg_output.status.success() {
        eprintln!(
            "Error generating SVG: {}",
            String::from_utf8_lossy(&svg_output.stderr)
        );
        return Err("Failed to generate SVG".into());
    }

    println!("SVG file generated: dependency-graph.svg");

    // Generate PNG from DOT file
    let png_output = std::process::Command::new("dot")
        .args([
            "-Tpng",
            "dependency-graph.dot",
            "-o",
            "dependency-graph.png",
        ])
        .output()?;

    if !png_output.status.success() {
        eprintln!(
            "Error generating PNG: {}",
            String::from_utf8_lossy(&png_output.stderr)
        );
        return Err("Failed to generate PNG".into());
    }

    println!("PNG file generated: dependency-graph.png");

    Ok(())
}
