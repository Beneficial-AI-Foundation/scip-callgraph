use serde_json::Value;
use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Read the JSON file
    let json_content = fs::read_to_string("unit_graph.json")?;
    let unit_graph: Value = serde_json::from_str(&json_content)?;

    // Calculate number of nodes
    let node_count = unit_graph["units"]
        .as_array()
        .map_or(0, |units| units.len());

    // Calculate total dependencies
    let mut total_dependencies = 0;
    if let Some(units) = unit_graph["units"].as_array() {
        for unit in units {
            total_dependencies += unit["dependencies"].as_array().map_or(0, |deps| deps.len());
        }
    }

    println!("Number of nodes (compilation units): {node_count}");
    println!("Total dependencies: {total_dependencies}");

    Ok(())
}
