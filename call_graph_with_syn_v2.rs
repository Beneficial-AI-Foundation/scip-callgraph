use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::error::Error;
use std::fs::File;
use std::io::Write;

use serde::{Serialize, Deserialize};
use syn::{
    visit::{self, Visit},
    ExprCall, ExprMethodCall, ItemFn, ItemStruct, ItemImpl,
};

use quote::ToTokens;
use serde_json::Value;
use std::sync::OnceLock;

use petgraph::graph::DiGraph;

static STANDARD_CRATES: OnceLock<HashSet<String>> = OnceLock::new();

fn init_standard_crates() -> HashSet<String> {
    // Attempt to load rustdoc_output.json (adjust path as needed)
    let json_str = std::fs::read_to_string("rustdoc_output.json").unwrap_or_default();
    let json: Value = serde_json::from_str(&json_str).unwrap_or_default();
    let mut set = HashSet::new();
    if let Some(ext) = json.get("external_crates") {
        for (_k, v) in ext.as_object().unwrap_or(&Default::default()) {
            if let Some(name) = v.get("name").and_then(|n| n.as_str()) {
                match name {
                    "std" | "core" | "alloc" => {
                        set.insert(name.to_string());
                    }
                    _ => {}
                }
            }
        }
    }
    set
}

fn is_standard_dependency(full_path: &str) -> bool {
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
    fn new() -> Self {
        Self { elements: Vec::new() }
    }

    fn merge(&mut self, other: CallGraph) {
        self.elements.extend(other.elements);
    }

    fn to_svg(&self, output_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut graph = DiGraph::<(String, String), String>::new();

        let mut node_indices = HashMap::new();

        for element in &self.elements {
            let node = graph.add_node((element.name.clone(), element.body.clone()));
            node_indices.insert(&element.name, node);
        }

        for element in &self.elements {
            if let Some(&source) = node_indices.get(&element.name) {
                for dependency in &element.dependencies {
                    if let Some(&target) = node_indices.get(dependency) {
                        graph.add_edge(source, target, "".to_string());
                    }
                }
            }
        }

        let dot_path = "call_graph.dot";
        let mut dot_content = String::new();
        dot_content.push_str("digraph G {\n");
        for node in graph.node_indices() {
            let (name, body) = &graph[node];
            dot_content.push_str(&format!(
                "    \"{}\" [label=\"{}\", tooltip=\"{}\"];\n",
                name,
                name,
                body.replace("\"", "\\\"").replace("\n", "\\n")
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

fn generate_dot(call_graph: &CallGraph, output_path: &str) -> Result<(), Box<dyn Error>> {
    let mut file = File::create(output_path)?;

    // Write DOT header
    writeln!(file, "digraph CallGraph {{")?;
    writeln!(file, "  rankdir=LR;")?;
    writeln!(file, "  node [shape=box, style=filled, fontname=\"Helvetica\"];")?;

    // Track nodes to avoid duplicates
    let mut nodes = HashSet::new();

    // Write nodes and edges
    for element in &call_graph.elements {
        let caller = &element.name;
        let callees = &element.dependencies;
        // Add caller node if not already added
        if nodes.insert(caller.clone()) {
            let color = match element.element_type.as_str() {
                "Function" => "lightblue",
                "Method" => "lightgreen",
                "Macro" => "lightyellow",
                _ => "white",
            };

            writeln!(
                file,
                "  \"{}\" [label=\"{}\\n({})\", fillcolor=\"{}\"];",
                caller.replace("\"", "\\\""),
                caller.replace("\"", "\\\""),
                element.body.replace("\"", "\\\""),
                color
            )?;
        }

        // Add callee nodes and edges
        for callee in callees {
            if nodes.insert(callee.clone()) {
                let color = match element.element_type.as_str() {
                    "Function" => "lightblue",
                    "Method" => "lightgreen",
                    "Macro" => "lightyellow",
                    _ => "white",
                };

                writeln!(
                    file,
                    "  \"{}\" [label=\"{}\\n({})\", fillcolor=\"{}\"];",
                    callee.replace("\"", "\\\""),
                    callee.replace("\"", "\\\""),
                    element.body.replace("\"", "\\\""),
                    color
                )?;
            }

            // Add edge
            writeln!(
                file,
                "  \"{}\" -> \"{}\";",
                caller.replace("\"", "\\\""),
                callee.replace("\"", "\\\"")
            )?;
        }
    }

    // Write DOT footer
    writeln!(file, "}}")?;

    Ok(())
}

struct FunctionCallVisitor {
    current_element: Option<String>,
    dependencies: HashMap<String, HashSet<String>>,
    bodies: HashMap<String, String>,
    elements: Vec<Element>,
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

    fn process_current_element(&mut self, element_type: &str) {
        if let Some(current_element) = &self.current_element {
            let dependencies: HashSet<String> = self.dependencies.remove(current_element).unwrap_or_default();
            let body = self.bodies.get(current_element).cloned().unwrap_or_default();

            self.elements.push(Element {
                name: current_element.clone(),
                element_type: element_type.to_string(),
                body,
                dependencies: dependencies.into_iter().collect(),
            });
        }
    }

    fn clean_body(body: &str) -> String {
        body.replace(" . ", ".")
            .replace(" .", ".")
            .replace(". ", ".")
            .replace(" (", "(")
    }

    fn store_body(&mut self, name: &str, body: &str) {
        self.bodies.insert(name.to_string(), Self::clean_body(body));
    }

    // Update add_dependency_with_rustdoc to resolve method calls to fully qualified names
    fn add_dependency_with_rustdoc(&mut self, full_path: &str) {
        if let Some(current) = &self.current_element {
            if current.trim() == full_path.trim() {
                return; // Skip self-dependency
            }
        }

        println!("Adding dependency: {}", full_path);
        // Resolve method calls to fully qualified names
        let resolved_path = if full_path.starts_with("self.") {
            // Replace `self.` with the current struct or impl type
            if let Some(current_struct) = self.current_element.as_ref() {
                let struct_name = current_struct.split("::").next().unwrap_or("");
                let path = full_path.replacen("self.", &format!("{}::", struct_name), 1);
                println!("Resolved path: {} struct_name: {}, full_path: {}", path, struct_name, full_path);
                path
            } else {
                full_path.to_string()
            }
        } else {
            full_path.to_string()
        };

        if !is_standard_dependency(&resolved_path) {
            let is_user_defined = self.user_defined.contains(&resolved_path);
            if is_user_defined {
                if let Some(current_element) = &self.current_element {
                    self.dependencies
                        .entry(current_element.clone())
                        .or_default()
                        .insert(resolved_path.to_string());
                }
            }
        }
    }

    fn to_call_graph(&self) -> CallGraph {
        CallGraph {
            elements: self.elements.clone(),
        }
    }
}

impl<'ast> Visit<'ast> for FunctionCallVisitor {
    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        let function_name = node.sig.ident.to_string();
        self.user_defined.insert(function_name.clone());
        self.process_current_element("function");
        self.current_element = Some(function_name.clone());
        let block = node.block.clone();
        let block_str = prettyplease::unparse(&syn::parse_file(&format!("fn dummy() {}", block.to_token_stream())).unwrap());
        let block_str = block_str.trim_start_matches("fn dummy()").trim();
        self.store_body(&function_name, block_str);
        visit::visit_item_fn(self, node);
        self.process_current_element("function");
        self.current_element = None;
    }

    fn visit_item_struct(&mut self, node: &'ast ItemStruct) {
        let struct_name = node.ident.to_string();
        self.user_defined.insert(struct_name.clone());
        self.process_current_element("struct");
        self.current_element = Some(struct_name.clone());
        let body: String = node.fields.to_token_stream().to_string();
        self.store_body(&struct_name, &body);
        self.process_current_element("struct");
        self.current_element = None;
    }

    fn visit_item_impl(&mut self, node: &'ast ItemImpl) {
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
                self.user_defined.insert(qualified_name.clone());
                self.process_current_element("function");
                self.current_element = Some(qualified_name.clone());
                let block = method.block.clone();
                let block_str = prettyplease::unparse(&syn::parse_file(&format!("fn dummy() {}", block.to_token_stream())).unwrap());
                let block_str = block_str.trim_start_matches("fn dummy()").trim();
                self.store_body(&qualified_name, block_str);
                self.visit_impl_item_fn(method);
                self.process_current_element("function");
                self.current_element = None;
            }
        }
    }

    fn visit_expr_call(&mut self, node: &'ast ExprCall) {
        if let Some(_current_element) = &self.current_element {
            if let syn::Expr::Path(expr_path) = &*node.func {
                let full_path = expr_path.to_token_stream().to_string();
                self.add_dependency_with_rustdoc(&full_path);
            }
        }
        visit::visit_expr_call(self, node);
    }

    // Visit method calls like `self.method()` or `obj.method()`
    fn visit_expr_method_call(&mut self, node: &'ast ExprMethodCall) {
        if let Some(_current_element) = &self.current_element {
            let method_name = node.method.to_string();
            let full_path = format!("self.{}", method_name); // Assume `self` for simplicity
            self.add_dependency_with_rustdoc(&full_path);
        }

        // Continue visiting the method call
        visit::visit_expr_method_call(self, node);
    }
}

fn process_folder(folder_path: &Path) -> Result<CallGraph, Box<dyn std::error::Error>> {
    let mut call_graph = CallGraph::new();

    for entry in fs::read_dir(folder_path)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            let sub_graph = process_folder(&path)?;
            call_graph.merge(sub_graph);
        } else if path.extension().map_or(false, |ext| ext == "rs") {
            let source = fs::read_to_string(&path)?;
            let syntax = syn::parse_file(&source)?;
            let mut visitor = FunctionCallVisitor::new();
            visitor.visit_file(&syntax);
            call_graph.merge(visitor.to_call_graph());
        }
    }

    Ok(call_graph)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <folder_path>", args[0]);
        process::exit(1);
    }

    let folder_path = PathBuf::from(&args[1]);
    if !folder_path.is_dir() {
        eprintln!("Error: {} is not a directory", folder_path.display());
        process::exit(1);
    }

    let call_graph = process_folder(&folder_path)?;

    let json = serde_json::to_string_pretty(&call_graph)?;
    fs::write("call_graph.json", &json)?;
    println!("Call graph written to call_graph.json");

    call_graph.to_svg("call_graph.svg")?;

    Ok(())
}