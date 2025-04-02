use std::fs;
use serde_json::Value;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Read the JSON file
    let json_str = fs::read_to_string("rustdoc_output.json")?;
    let json: Value = serde_json::from_str(&json_str)?;

    // Print external crates
    println!("External Dependencies:");
    println!("--------------------");
    if let Some(crates) = json["external_crates"].as_object() {
        for (_, crate_info) in crates {
            println!("- {}", crate_info["name"].as_str().unwrap_or("unknown"));
        }
    }

    // Print internal dependencies
    println!("\nInternal Dependencies:");
    println!("--------------------");
    if let Some(index) = json["index"].as_object() {
        for (id, item) in index {
            let name = item["name"].as_str().unwrap_or("anonymous");
            let links = item["links"].as_object().map(|l| l.len()).unwrap_or(0);
            if links > 0 {
                println!("- {} (ID: {}) has {} links", name, id, links);
                if let Some(link_obj) = item["links"].as_object() {
                    for (link_name, link_id) in link_obj {
                        println!("  → {} (links to ID: {})", link_name, link_id);
                    }
                }
            }
        }
    }

    Ok(())
}
