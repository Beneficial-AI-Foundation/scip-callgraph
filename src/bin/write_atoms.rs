use scip_callgraph::scip_to_call_graph_json::{
    build_call_graph, parse_scip_json, write_call_graph_as_atoms_json,
};

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
            eprintln!("Failed to parse SCIP JSON: {e}");
            std::process::exit(1);
        }
    };
    let call_graph = build_call_graph(&scip_index);
    if let Err(e) = write_call_graph_as_atoms_json(&call_graph, output_path) {
        eprintln!("Failed to write atoms JSON: {e}");
        std::process::exit(1);
    }
    println!("Atoms JSON written to {output_path}");
}
