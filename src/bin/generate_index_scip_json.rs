use scip_callgraph::scip_utils::generate_scip_json_index;
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <path_to_folder>", args[0]);
        eprintln!("Example: {} /path/to/rust/project", args[0]);
        std::process::exit(1);
    }

    let folder_path = &args[1];
    
    match generate_scip_json_index(folder_path) {
        Ok(output_file) => {
            println!("\nDone! Output file: {}", output_file);
            Ok(())
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

