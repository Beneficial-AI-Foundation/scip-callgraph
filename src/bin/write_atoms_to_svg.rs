use rust_analyzer_test::scip_to_call_graph_json::{parse_scip_json, build_call_graph, generate_call_graph_svg};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} <input_scip_json> <output_atoms_json>", args[0]);
        std::process::exit(1);
    }
    let input_path = &args[1];
    let output_path = &args[2];

    let scip_index = match parse_scip_json(input_path) {
        Ok(idx) => idx,
        Err(e) => {
            eprintln!("Failed to parse SCIP JSON: {}", e);
            std::process::exit(1);
        }
    };
    let call_graph = build_call_graph(&scip_index);

    if let Err(e) = generate_call_graph_svg(&call_graph, output_path) {
        eprintln!("Failed to write atoms to SVG: {}", e);
        std::process::exit(1);
    }
    println!("Atoms SVG written to {}", output_path);
}