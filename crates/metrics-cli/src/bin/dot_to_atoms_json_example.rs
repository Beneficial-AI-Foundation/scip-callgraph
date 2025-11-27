fn main() {
    let dot_file = "call_graph.dot";
    let json_file = "output.json";
    let scip_json = "/home/lacra/git_repos/scip-callgraph/src/index_scip.json";
    let src_root = "/home/lacra/git_repos/scip-callgraph/src";
    match scip_core::scip_call_graph::dot_to_atoms_json_with_body(
        dot_file, json_file, scip_json, src_root,
    ) {
        Ok(()) => println!("Successfully generated {json_file} from {dot_file}"),
        Err(e) => eprintln!("Error: {e}"),
    }
}
