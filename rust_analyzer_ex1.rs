use std::fs;
use std::path::Path;
use syn::{visit::Visit, ItemFn};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Path to your Rust file
    let file_path = Path::new("./src/main.rs");

    // Read the file content
    let file_content = fs::read_to_string(file_path)?;

    // Parse the file content
    let syntax_tree = syn::parse_file(&file_content)?;
    println!("Parsed syntax tree for {}", file_path.display());

    // Extract function definitions
    let mut visitor = FunctionVisitor::new();
    visitor.visit_file(&syntax_tree);

    println!("\nFunctions in {}:", file_path.display());
    for func in visitor.functions {
        println!("  - {} at line {}", func.sig.ident, func.line);
    }

    Ok(())
}

// Visitor to extract function definitions
struct FunctionVisitor {
    functions: Vec<FunctionInfo>,
}

impl FunctionVisitor {
    fn new() -> Self {
        Self { functions: Vec::new() }
    }
}

impl<'ast> Visit<'ast> for FunctionVisitor {
    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        let line = node.sig.ident.span().location().line;
        self.functions.push(FunctionInfo {
            sig: node.sig.clone(),
            line,
        });
    }
}

// Struct to store function information
struct FunctionInfo {
    sig: syn::Signature,
    line: usize,
}