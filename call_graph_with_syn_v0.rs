use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::process;

use petgraph::graph::{DiGraph, NodeIndex};
use serde::{Serialize, Deserialize};
use syn::{
    visit::{self, Visit},
    ExprCall, ExprMethodCall, ItemFn, Path as SynPath,
};

#[derive(Serialize, Deserialize, Debug)]
struct Node {
    id: usize,
    name: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct Edge {
    source: usize,
    target: usize,
}

#[derive(Serialize, Deserialize, Debug)]
struct CallGraph {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
}

struct FunctionCallVisitor {
    // Current function we're visiting
    current_function: Option<String>,
    
    // Map of function names to node indices
    function_map: HashMap<String, NodeIndex>,
    
    // Keep track of calls between functions
    call_graph: DiGraph<String, ()>,
    
    // Keep track of function calls found in the current function
    calls: HashSet<String>,
}

impl FunctionCallVisitor {
    fn new() -> Self {
        FunctionCallVisitor {
            current_function: None,
            function_map: HashMap::new(),
            call_graph: DiGraph::new(),
            calls: HashSet::new(),
        }
    }
    
    // Add a function to the graph if it doesn't exist
    fn add_function(&mut self, name: &str) -> NodeIndex {
        *self.function_map.entry(name.to_string()).or_insert_with(|| {
            self.call_graph.add_node(name.to_string())
        })
    }
    
    // Extract function name from a path expression
    fn extract_path_ident(&self, path: &SynPath) -> Option<String> {
        path.segments.last().map(|segment| segment.ident.to_string())
    }
    
    // Process collected calls when we exit a function
    fn process_current_calls(&mut self) {
        if let Some(current_fn) = &self.current_function {
            let current_fn_clone = current_fn.clone(); // Clone to avoid borrowing conflict
            let caller_idx = self.add_function(&current_fn_clone);
            
            // Create a copy of the calls to avoid borrowing issues
            let calls_copy: Vec<String> = self.calls.iter().cloned().collect();
            
            // Add edges for all calls made from this function
            for callee in calls_copy {
                let callee_idx = self.add_function(&callee);
                // Avoid duplicate edges
                if !self.call_graph.contains_edge(caller_idx, callee_idx) {
                    self.call_graph.add_edge(caller_idx, callee_idx, ());
                }
            }
            
            // Clear the calls for the next function
            self.calls.clear();
        }
    }
    
    // Convert the graph to the JSON structure
    fn to_call_graph(&self) -> CallGraph {
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        
        // Add all nodes with their indices
        for (id, node_idx) in self.function_map.values().enumerate() {
            nodes.push(Node {
                id,
                name: self.call_graph[*node_idx].clone(),
            });
        }
        
        // Create a mapping from NodeIndex to our sequential IDs
        let node_to_id: HashMap<NodeIndex, usize> = self.function_map.values()
            .enumerate()
            .map(|(id, &node_idx)| (node_idx, id))
            .collect();
        
        // Add all edges
        for edge in self.call_graph.edge_indices() {
            if let Some((source, target)) = self.call_graph.edge_endpoints(edge) {
                edges.push(Edge {
                    source: *node_to_id.get(&source).unwrap(),
                    target: *node_to_id.get(&target).unwrap(),
                });
            }
        }
        
        CallGraph { nodes, edges }
    }
}

impl<'ast> Visit<'ast> for FunctionCallVisitor {
    // Visit function definitions
    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        // Process any pending calls from the previous function
        self.process_current_calls();
        
        // Set the current function
        self.current_function = Some(node.sig.ident.to_string());
        
        // Visit the function body to find calls
        visit::visit_item_fn(self, node);
        
        // Process the calls we found in this function
        self.process_current_calls();
        
        // Clear the current function
        self.current_function = None;
    }
    
    // Visit direct function calls like `foo()`
    fn visit_expr_call(&mut self, node: &'ast ExprCall) {
        if let Some(current_fn) = &self.current_function {
            // Extract the function name if it's a path (most common case)
            if let syn::Expr::Path(expr_path) = &*node.func {
                if let Some(func_name) = self.extract_path_ident(&expr_path.path) {
                    // Don't record self-calls
                    if func_name != *current_fn {
                        self.calls.insert(func_name);
                    }
                }
            }
        }
        
        // Continue visiting other parts of the call
        visit::visit_expr_call(self, node);
    }
    
    // Visit method calls like `obj.method()`
    fn visit_expr_method_call(&mut self, node: &'ast ExprMethodCall) {
        if let Some(current_fn) = &self.current_function {
            let method_name = node.method.to_string();
            // Don't record self-calls (though this is an approximation for methods)
            if method_name != *current_fn {
                self.calls.insert(method_name);
            }
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