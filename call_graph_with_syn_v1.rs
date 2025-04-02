use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::process;

use serde::{Serialize, Deserialize};
use syn::{
    visit::{self, Visit},
    ExprCall, ExprMethodCall, ItemFn, ItemStruct, ItemImpl, Path as SynPath, 
};
use quote::ToTokens;
use serde_json::Value;
use std::sync::OnceLock; // Rust 1.70+ (or use lazy_static)

static STANDARD_CRATES: OnceLock<HashSet<String>> = OnceLock::new();

fn init_standard_crates() -> HashSet<String> {
    // Attempt to load rustdoc_output.json (adjust path as needed)
    let json_str = std::fs::read_to_string("rustdoc_output.json").unwrap_or_default();
    let json: Value = serde_json::from_str(&json_str).unwrap_or_default();
    let mut set = HashSet::new();
    if let Some(ext) = json.get("external_crates") {
        // For this example, we consider "std", "core", "alloc" as standard,
        // but you could also filter based on other properties.
        for (_k, v) in ext.as_object().unwrap_or(&Default::default()) {
            if let Some(name) = v.get("name").and_then(|n| n.as_str()) {
                match name {
                    "std" | "core" | "alloc" => { set.insert(name.to_string()); },
                    _ => {}
                }
            }
        }
    }
    set
}

fn is_standard_dependency(full_path: &str) -> bool {
    // Assume full_path is something like "std::vec::Vec" or "helper::utils::print_message".
    // Split the path by "::" and check the first segment.
    if let Some(first) = full_path.split("::").next() {
        STANDARD_CRATES.get().map_or(false, |set| set.contains(first))
    } else {
        false
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Element {
    name: String,
    element_type: String,
    body: String,
    dependencies: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct CallGraph {
    elements: Vec<Element>,
}

struct FunctionCallVisitor {
    // Current function or struct we're visiting
    current_element: Option<String>,
    
    // Map of element names to their dependencies
    dependencies: HashMap<String, HashSet<String>>,
    
    // Map of element names to their bodies
    bodies: HashMap<String, String>,
    
    // List of elements (functions, structs, etc.)
    elements: Vec<Element>,
}

impl FunctionCallVisitor {
    fn new() -> Self {
        STANDARD_CRATES.get_or_init(init_standard_crates);
        Self {
            current_element: None,
            dependencies: HashMap::new(),
            bodies: HashMap::new(),
            elements: Vec::new(),
        }
    }
    
    // Process collected dependencies when we exit an element
    fn process_current_element(&mut self, element_type: &str) {
        if let Some(current_element) = &self.current_element {
            let mut dependencies: HashSet<String> = self.dependencies.remove(current_element).unwrap_or_default();
            // Remove self dependency
            dependencies.remove(current_element);
            let body = self.bodies.remove(current_element).unwrap_or_default();
            
            self.elements.push(Element {
                name: current_element.clone(),
                element_type: element_type.to_string(),
                body,
                dependencies: dependencies.into_iter().collect(),
            });
        }
    }
    
    // Store the body of the current element
    fn store_body(&mut self, name: &str, body: &str) {
        self.bodies.insert(name.to_string(), body.to_string());
    }
    
    // Add a dependency to the current element
    fn add_dependency_with_rustdoc(&mut self, full_path: &str) {
        if !is_standard_dependency(full_path) {
            if let Some(current_element) = &self.current_element {
                self.dependencies
                    .entry(current_element.clone())
                    .or_default()
                    .insert(full_path.to_string());
            }
        }
    }
    
    // Convert the graph to the JSON structure
    fn to_call_graph(&self) -> CallGraph {
        CallGraph {
            elements: self.elements.clone(),
        }
    }
}

impl<'ast> Visit<'ast> for FunctionCallVisitor {
    // Visit function definitions
    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        // Process any pending element
        self.process_current_element("function");
        
        // Set the current function
        let function_name = node.sig.ident.to_string();
        self.current_element = Some(function_name.clone());
        
        // Store the function body
        self.store_body(&function_name, &node.block.to_token_stream().to_string());
        
        // Visit the function body to find dependencies
        visit::visit_item_fn(self, node);
        
        // Process the current function
        self.process_current_element("function");
        
        // Clear the current element
        self.current_element = None;
    }
    
    // Visit struct definitions
    fn visit_item_struct(&mut self, node: &'ast ItemStruct) {
        // Process any pending element
        self.process_current_element("struct");
        
        // Set the current struct
        let struct_name = node.ident.to_string();
        self.current_element = Some(struct_name.clone());
        
        // Store the struct body
        self.store_body(&struct_name, &node.fields.to_token_stream().to_string());
        
        // Process the current struct
        self.process_current_element("struct");
        
        // Clear the current element
        self.current_element = None;
    }

    // Visit impl blocks
    fn visit_item_impl(&mut self, node: &'ast ItemImpl) {
        // Visit each function within the impl block
        for item in &node.items {
            if let syn::ImplItem::Fn(method) = item {
                // Process any pending element
                self.process_current_element("function");

                // Set the current function
                let function_name = method.sig.ident.to_string();
                self.current_element = Some(function_name.clone());

                // Store the function body
                self.store_body(&function_name, &method.block.to_token_stream().to_string());

                // Visit the function body to find dependencies using visit_impl_item_fn
                self.visit_impl_item_fn(method);

                // Process the current function
                self.process_current_element("function");

                // Clear the current element
                self.current_element = None;
            }
        }
    }
    
    // Visit direct function calls like `foo()`
    fn visit_expr_call(&mut self, node: &'ast ExprCall) {
        if let Some(_current_element) = &self.current_element {
            // Extract the function name if it's a path (most common case)
            if let syn::Expr::Path(expr_path) = &*node.func {
                let full_path = expr_path.to_token_stream().to_string(); 
                self.add_dependency_with_rustdoc(&full_path);
            }
        }
        
        // Continue visiting other parts of the call
        visit::visit_expr_call(self, node);
    }
    
    // Visit method calls like `obj.method()`
    fn visit_expr_method_call(&mut self, node: &'ast ExprMethodCall) {
        if let Some(_current_element) = &self.current_element {
            let method_name = node.method.to_string();
            // Add the method as a dependency
            self.add_dependency_with_rustdoc(&method_name);
        }
        
        // Continue visiting the method call
        visit::visit_expr_method_call(self, node);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get the file path from command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <rust_file_path>", args[0]);
        process::exit(1);
    }
    
    let file_path = &args[1];
    
    // Read the file content
    let source = fs::read_to_string(file_path)?;
    
    // Parse the file with syn
    let syntax = syn::parse_file(&source)?;
    
    // Create our visitor
    let mut visitor = FunctionCallVisitor::new();
    
    // Visit the syntax tree
    visitor.visit_file(&syntax);
    
    // Convert to our JSON structure
    let call_graph = visitor.to_call_graph();
    
    // Output as JSON
    let json = serde_json::to_string_pretty(&call_graph)?;
    println!("{}", json);
    
    Ok(())
}