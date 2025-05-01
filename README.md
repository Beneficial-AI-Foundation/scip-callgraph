# rust-analyzer-test

## Prerequirements

Install:
- [rust-analyzer](https://rust-analyzer.github.io/book/installation.html) it is used to generate a scip output file
- [scip](https://github.com/sourcegraph/scip) it is used to generate a JSON from the scip output file  

## How to run

- `cargo run --bin generate_call_graph_dot <path_to_rust_repo> <output_file>.dot`