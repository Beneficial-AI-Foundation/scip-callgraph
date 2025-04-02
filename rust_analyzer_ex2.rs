use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::fs;

// Normally we'd use the rust-analyzer crate directly
// For this example, we'll create a simplified version that mimics the real usage

// Simplified structures to represent what rust-analyzer would provide
struct Analysis {
    project_root: PathBuf,
    file_contents: HashMap<String, String>,
    function_definitions: HashMap<String, Vec<FunctionDef>>,
    function_calls: HashMap<String, Vec<FunctionCall>>,
}

struct FunctionDef {
    name: String,
    file: String,
    line: usize,
    calls: Vec<String>, // Names of functions this function calls
    body: String,       // Function body
}

struct FunctionCall {
    from: String,
    to: String,
    line: usize,
}

struct DependencyGraph {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
}

struct Node {
    id: String,
    name: String,
    path: String,
    node_type: String,
    body: String, // Function body
}

struct Edge {
    source: String,
    target: String,
    edge_type: String,
    weight: i32,
}

impl Analysis {
    // Create a new analysis instance for the project
    fn new(project_path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let project_root = project_path.to_path_buf();
        
        // Load file contents
        let mut file_contents = HashMap::new();
        let main_rs_path = project_root.join("src/main.rs");
        
        let main_rs_content = fs::read_to_string(&main_rs_path)?;
        file_contents.insert("src/main.rs".to_string(), main_rs_content.clone());
        
        // Normally rust-analyzer would parse the code and extract this information
        // Here we'll hard-code it based on our test project
        let mut function_definitions = HashMap::new();
        let mut function_calls = HashMap::new();
        
        // Define functions in main.rs
        function_definitions.insert("src/main.rs".to_string(), vec![
            FunctionDef {
                name: "main".to_string(),
                file: "src/main.rs".to_string(),
                line: 4,
                calls: vec![
                    "helper::utils::print_message".to_string(),
                    "calculate_value".to_string(),
                    "process_data".to_string(),
                ],
                body: Self::extract_function_body(&main_rs_content, "main"),
            },
            FunctionDef {
                name: "calculate_value".to_string(),
                file: "src/main.rs".to_string(),
                line: 14,
                calls: vec![
                    "helper::math::multiply".to_string(),
                    "helper::math::square".to_string(),
                ],
                body: Self::extract_function_body(&main_rs_content, "calculate_value"),
            },
            FunctionDef {
                name: "process_data".to_string(),
                file: "src/main.rs".to_string(),
                line: 21,
                calls: vec![
                    "helper::data::transform_data".to_string(),
                    "helper::data::analyze_data".to_string(),
                ],
                body: Self::extract_function_body(&main_rs_content, "process_data"),
            },
        ]);
        
        // Define functions in helper modules
        let helper_rs_path = project_root.join("src/helper.rs");
        let helper_rs_content = fs::read_to_string(&helper_rs_path)?;
        file_contents.insert("src/helper.rs".to_string(), helper_rs_content.clone());
        
        function_definitions.insert("src/helper.rs".to_string(), vec![
            FunctionDef {
                name: "helper::utils::print_message".to_string(),
                file: "src/helper.rs".to_string(),
                line: 31,
                calls: vec![],
                body: Self::extract_function_body(&helper_rs_content, "helper::utils::print_message"),
            },
            FunctionDef {
                name: "helper::utils::format_string".to_string(),
                file: "src/helper.rs".to_string(),
                line: 35,
                calls: vec![],
                body: Self::extract_function_body(&helper_rs_content, "helper::utils::format_string"),
            },
            FunctionDef {
                name: "helper::math::add".to_string(),
                file: "src/helper.rs".to_string(),
                line: 42,
                calls: vec![],
                body: Self::extract_function_body(&helper_rs_content, "helper::math::add"),
            },
            FunctionDef {
                name: "helper::math::multiply".to_string(),
                file: "src/helper.rs".to_string(),
                line: 46,
                calls: vec![],
                body: Self::extract_function_body(&helper_rs_content, "helper::math::multiply"),
            },
            FunctionDef {
                name: "helper::math::square".to_string(),
                file: "src/helper.rs".to_string(),
                line: 50,
                calls: vec![],
                body: Self::extract_function_body(&helper_rs_content, "helper::math::square"),
            },
            FunctionDef {
                name: "helper::data::transform_data".to_string(),
                file: "src/helper.rs".to_string(),
                line: 56,
                calls: vec![],
                body: Self::extract_function_body(&helper_rs_content, "helper::data::transform_data"),
            },
            FunctionDef {
                name: "helper::data::analyze_data".to_string(),
                file: "src/helper.rs".to_string(),
                line: 60,
                calls: vec![],
                body: Self::extract_function_body(&helper_rs_content, "helper::data::analyze_data"),
            },
            FunctionDef {
                name: "helper::data::filter_data".to_string(),
                file: "src/helper.rs".to_string(),
                line: 64,
                calls: vec![],
                body: Self::extract_function_body(&helper_rs_content, "helper::data::filter_data"),
            },
        ]);
        
        // Record function calls
        function_calls.insert("src/main.rs".to_string(), vec![
            FunctionCall {
                from: "main".to_string(),
                to: "helper::utils::print_message".to_string(),
                line: 7,
            },
            FunctionCall {
                from: "main".to_string(),
                to: "calculate_value".to_string(),
                line: 10,
            },
            FunctionCall {
                from: "main".to_string(),
                to: "process_data".to_string(),
                line: 13,
            },
            FunctionCall {
                from: "calculate_value".to_string(),
                to: "helper::math::multiply".to_string(),
                line: 19,
            },
            FunctionCall {
                from: "calculate_value".to_string(),
                to: "helper::math::square".to_string(),
                line: 19,
            },
            FunctionCall {
                from: "process_data".to_string(),
                to: "helper::data::transform_data".to_string(),
                line: 25,
            },
            FunctionCall {
                from: "process_data".to_string(),
                to: "helper::data::analyze_data".to_string(),
                line: 26,
            },
        ]);
        
        Ok(Analysis {
            project_root,
            file_contents,
            function_definitions,
            function_calls,
        })
    }
    
    // Helper function to extract function body (very basic implementation)
    fn extract_function_body(file_content: &str, function_name: &str) -> String {
        let start_pattern = format!("fn {} (", function_name.replace("helper::", ""));
        if let Some(start_index) = file_content.find(&start_pattern) {
            let start_body = file_content[start_index..].find('{').map(|x| x + start_index + 1).unwrap_or(file_content.len());
            
            // Very basic: find the next function definition or end of file
            let end_pattern = "fn ";
            let end_index = file_content[start_body..].find(end_pattern).map(|x| x + start_body).unwrap_or(file_content.len());
            
            file_content[start_body..end_index].trim().to_string()
        } else {
            String::new()
        }
    }

    // Get all function definitions in a file
    fn get_functions_in_file(&self, file_path: &str) -> Vec<&FunctionDef> {
        self.function_definitions
            .get(file_path)
            .map_or(vec![], |defs| defs.iter().collect())
    }
    
    // Get all function calls in a file
    fn get_calls_in_file(&self, file_path: &str) -> Vec<&FunctionCall> {
        self.function_calls
            .get(file_path)
            .map_or(vec![], |calls| calls.iter().collect())
    }
    
    // Build a dependency graph for the project
    fn build_dependency_graph(&self) -> DependencyGraph {
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        let mut node_ids = HashMap::new();
        
        // Create nodes for all functions
        let mut node_id = 0;
        for (file, functions) in &self.function_definitions {
            for func in functions {
                let id = format!("node_{}", node_id);
                node_ids.insert(func.name.clone(), id.clone());
                
                nodes.push(Node {
                    id,
                    name: func.name.clone(),
                    path: file.clone(),
                    node_type: "function".to_string(),
                    body: func.body.clone(),
                });
                
                node_id += 1;
            }
        }
        
        // Create edges for all function calls
        for (_, calls) in &self.function_calls {
            for call in calls {
                if let (Some(source_id), Some(target_id)) = (
                    node_ids.get(&call.from),
                    node_ids.get(&call.to),
                ) {
                    edges.push(Edge {
                        source: source_id.clone(),
                        target: target_id.clone(),
                        edge_type: "calls".to_string(),
                        weight: 1,
                    });
                }
            }
        }
        
        DependencyGraph { nodes, edges }
    }
}

// A simplified implementation of serde::Serialize for our types
// In a real implementation, you'd use #[derive(Serialize)]
impl DependencyGraph {
    fn to_json(&self) -> String {
        let mut json = String::from("{\n  \"nodes\": [\n");
        
        // Add nodes
        for (i, node) in self.nodes.iter().enumerate() {
            json.push_str(&format!(
                "    {{\"id\": \"{}\", \"name\": \"{}\", \"path\": \"{}\", \"type\": \"{}\", \"body\": \"{}\"}}",
                node.id, node.name, node.path, node.node_type, node.body.replace("\\", "\\\\").replace("\"", "\\\"").replace("\n", "\\n")
            ));
            
            if i < self.nodes.len() - 1 {
                json.push_str(",\n");
            } else {
                json.push_str("\n");
            }
        }
        
        json.push_str("  ],\n  \"edges\": [\n");
        
        // Add edges
        for (i, edge) in self.edges.iter().enumerate() {
            json.push_str(&format!(
                "    {{\"source\": \"{}\", \"target\": \"{}\", \"type\": \"{}\", \"weight\": {}}}",
                edge.source, edge.target, edge.edge_type, edge.weight
            ));
            
            if i < self.edges.len() - 1 {
                json.push_str(",\n");
            } else {
                json.push_str("\n");
            }
        }
        
        json.push_str("  ]\n}");
        json
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Path to the project (in a real scenario, this would be an actual path)
    let project_path = Path::new(".");
    
    println!("Analyzing Rust project...");
    
    // Create an analysis instance
    let analysis = Analysis::new(project_path)?;
    
    // Analyze main.rs
    let main_rs = "src/main.rs";
    let functions = analysis.get_functions_in_file(main_rs);
    
    println!("\nFunctions in {}:", main_rs);
    for func in functions {
        println!("  - {} at line {}", func.name, func.line);
    }
    
    // Get function calls
    let calls = analysis.get_calls_in_file(main_rs);
    
    println!("\nFunction calls in {}:", main_rs);
    for call in calls {
        println!("  - From {} to {} at line {}", call.from, call.to, call.line);
    }
    
    // Generate dependency graph
    println!("\nBuilding dependency graph...");
    let dependency_graph = analysis.build_dependency_graph();
    
    // Convert to JSON
    let json = dependency_graph.to_json();
    fs::write("dependency_graph.json", &json)?;
    println!("Dependency graph written to dependency_graph.json");
    
    // Print a sample of the JSON
    println!("\nJSON sample:");
    let lines: Vec<&str> = json.lines().take(10).collect();
    for line in lines {
        println!("{}", line);
    }
    println!("...");
    
    Ok(())
}