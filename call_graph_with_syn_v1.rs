use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::process;

use serde::{Serialize, Deserialize};
use syn::{
    visit::{self, Visit},
    ExprCall, ExprMethodCall, ItemFn, ItemStruct, ItemImpl,  
};

use quote::ToTokens;
use serde_json::Value;
use std::sync::OnceLock; // Rust 1.70+ (or use lazy_static)
use petgraph::dot::{Dot, Config};
use petgraph::graph::DiGraph;

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

impl CallGraph {
    // Generate an SVG visualization of the call graph with tooltips for node bodies
    fn to_svg(&self, output_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Create a directed graph
        let mut graph = DiGraph::<(String, String), String>::new(); // Use `(String, String)` for node attributes

        // Map to store node indices
        let mut node_indices = HashMap::new();

        // Add nodes to the graph
        for element in &self.elements {
            let node = graph.add_node((
                element.name.clone(),
                element.body.clone(), // Store the body as part of the node attributes
            ));
            node_indices.insert(&element.name, node);
        }

        // Add edges to the graph
        for element in &self.elements {
            if let Some(&source) = node_indices.get(&element.name) {
                for dependency in &element.dependencies {
                    if let Some(&target) = node_indices.get(dependency) {
                        graph.add_edge(source, target, "".to_string()); // Use an empty string as the edge label
                    }
                }
            }
        }

        // Generate the DOT representation with tooltips
        let _dot = Dot::with_config(&graph, &[Config::EdgeNoLabel]);

        // Write the DOT representation to a temporary file
        let dot_path = "call_graph.dot";
        let mut dot_content = String::new();
        dot_content.push_str("digraph G {\n");
        for node in graph.node_indices() {
            let (name, body) = &graph[node];
            dot_content.push_str(&format!(
                "    \"{}\" [label=\"{}\", tooltip=\"{}\"];\n",
                name,
                name,
                body.replace("\"", "\\\"").replace("\n", "\\n") // Escape quotes and newlines
            ));
        }
        for edge in graph.edge_indices() {
            let (source, target) = graph.edge_endpoints(edge).unwrap();
            dot_content.push_str(&format!(
                "    \"{}\" -> \"{}\";\n",
                graph[source].0, graph[target].0
            ));
        }
        dot_content.push_str("}\n");
        std::fs::write(dot_path, dot_content)?;

        // Use `dot` command to generate the SVG
        let output = std::process::Command::new("dot")
            .args(["-Tsvg", dot_path, "-o", output_path])
            .output()?;

        if !output.status.success() {
            eprintln!("Error generating SVG: {}", String::from_utf8_lossy(&output.stderr));
            return Err("Failed to generate SVG".into());
        }

        println!("SVG visualization written to {}", output_path);
        Ok(())
    }
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

    // NEW: to store user defined function/struct names
    user_defined: HashSet<String>,
}

impl FunctionCallVisitor {
    fn new() -> Self {
        STANDARD_CRATES.get_or_init(init_standard_crates);
        Self {
            current_element: None,
            dependencies: HashMap::new(),
            bodies: HashMap::new(),
            elements: Vec::new(),
            user_defined: HashSet::new(),
        }
    }
    
    // Process collected dependencies when we exit an element
    fn process_current_element(&mut self, element_type: &str) {
        if let Some(current_element) = &self.current_element {
            // Retrieve dependencies
            let dependencies: HashSet<String> = self.dependencies.remove(current_element).unwrap_or_default();
            // Retrieve the body without removing it
            let body = self.bodies.get(current_element).cloned().unwrap_or_default();
            
            // Push the element into the elements list
            self.elements.push(Element {
                name: current_element.clone(),
                element_type: element_type.to_string(),
                body,
                dependencies: dependencies.into_iter().collect(),
            });
        }
    }
    
    // Updated helper to remove extra spaces around '.' and preceding '(' while preserving newlines.
    fn clean_body(body: &str) -> String {
        body.replace(" . ", ".")
            .replace(" .", ".")
            .replace(". ", ".")
            .replace(" (", "(")
    }

    // Update store_body to use clean_body.
    fn store_body(&mut self, name: &str, body: &str) {
        self.bodies.insert(name.to_string(), Self::clean_body(body));
    }
    
    // Update add_dependency_with_rustdoc to skip self dependency.
    fn add_dependency_with_rustdoc(&mut self, full_path: &str) {
        //println!("Adding dependency: {}", full_path);
        //println!("user_defined: {:?}", self.user_defined);
        // Skip if dependency is same as current element
        if let Some(current) = &self.current_element {
            if current.trim() == full_path.trim() {
                return;
            }
        }
        if !is_standard_dependency(full_path) {
            // Check if any user-defined name either equals or ends with " :: <dependency>"
            let is_user_defined = self.user_defined.contains(full_path);
            if is_user_defined {
                if let Some(current_element) = &self.current_element {
                    self.dependencies
                        .entry(current_element.clone())
                        .or_default()
                        .insert(full_path.to_string());
                }
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
    // Visit function definitions: record name as user defined.
    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        let function_name = node.sig.ident.to_string();
        self.user_defined.insert(function_name.clone());
        self.process_current_element("function");
        self.current_element = Some(function_name.clone());
        // Join tokens from function block with a newline after each token.
        let block = node.block.clone();
        let block_str = prettyplease::unparse(&syn::parse_file(&format!("fn dummy() {}", block.to_token_stream())).unwrap());
        let block_str = block_str.trim_start_matches("fn dummy()").trim();
        self.store_body(&function_name, block_str);
        visit::visit_item_fn(self, node);
        self.process_current_element("function");
        self.current_element = None;
    }
    
    // Visit struct definitions: record struct name as user defined.
    fn visit_item_struct(&mut self, node: &'ast ItemStruct) {
        let struct_name = node.ident.to_string();
        self.user_defined.insert(struct_name.clone());
        // Process any pending element
        self.process_current_element("struct");
        
        // Set the current struct
        self.current_element = Some(struct_name.clone());
        
        // Store the struct body
        let body: String = node.fields.to_token_stream().to_string();
        self.store_body(&struct_name, &body);
        
        // Process the current struct
        self.process_current_element("struct");
        
        // Clear the current element
        self.current_element = None;
    }

    // Visit impl blocks: record method using qualified name "ImplType :: method"
    fn visit_item_impl(&mut self, node: &'ast ItemImpl) {
        // Attempt to extract type name of the impl (if present)
        let impl_type = if let syn::Type::Path(type_path) = &*node.self_ty {
            type_path.path.segments.last().map(|seg| seg.ident.to_string())
        } else {
            None
        };

        for item in &node.items {
            if let syn::ImplItem::Fn(method) = item {
                let method_name = method.sig.ident.to_string();
                let qualified_name = if let Some(ref tname) = impl_type {
                    format!("{} :: {}", tname, method_name)
                } else {
                    method_name.clone()
                };
                // Insert the qualified name into user_defined.
                self.user_defined.insert(qualified_name.clone());
                // Process any pending element
                self.process_current_element("function");

                // Set current function to the qualified name.
                self.current_element = Some(qualified_name.clone());

                // Store function body, using the qualified name.
            
                let block = method.block.clone();
                let block_str = prettyplease::unparse(&syn::parse_file(&format!("fn dummy() {}", block.to_token_stream())).unwrap());
                let block_str = block_str.trim_start_matches("fn dummy()").trim();
                self.store_body(&qualified_name, block_str);

                // Visit the function body to find dependencies using visit_impl_item_fn
                self.visit_impl_item_fn(method);

                // Process the current function
                self.process_current_element("function");

                // Clear current element
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
    // NEW: write the JSON to a file
    fs::write("call_graph.json", &json)?;
    println!("Call graph written to call_graph.json");

    // Generate the SVG visualization
    call_graph.to_svg("call_graph.svg")?;
    
    Ok(())
}