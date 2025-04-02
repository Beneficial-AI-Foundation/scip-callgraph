use std::path::Path;
use ra_ap_base::FileId;
use ra_ap_ide::{
    AnalysisHost,
    FilePosition,
    RootDatabase,
};
use ra_ap_project_model::{
    CargoConfig,
    ProjectManifest,
    ProjectWorkspace,
};
use ra_ap_syntax::{
    AstNode,
    SourceFile,
    ast,
};
use ra_ap_vfs::Vfs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize analysis host
    let mut host = AnalysisHost::default();
    
    // Load the project
    let manifest_path = ProjectManifest::discover_single(
        Path::new(".")
    )?;
    
    let workspace = ProjectWorkspace::load(
        manifest_path,
        &CargoConfig::default(),
        &|_| {},
    )?;
    
    // Create a new VFS
    let mut vfs = Vfs::default();
    
    // Load workspace files
    for root in workspace.iter() {
        if let Some(path) = root.include_dir() {
            let main_rs_path = path.join("src/main.rs");
            if main_rs_path.exists() {
                let content = std::fs::read_to_string(&main_rs_path)?;
                vfs.add_file_overlay(main_rs_path, content);
            }
        }
    }
    
    // Get analysis instance
    let analysis = host.analysis();
    
    // Parse a specific file
    let main_rs = FileId::from_raw(0);
    let position = FilePosition { 
        file_id: main_rs, 
        offset: 0.into() 
    };
    
    if let Ok(source) = analysis.parse(main_rs) {
        if let Ok(source_file) = SourceFile::parse(&source.text()) {
            // Find all function definitions
            println!("Functions found:");
            for node in source_file
                .syntax()
                .descendants()
                .filter_map(ast::Fn::cast)
            {
                if let Some(name) = node.name() {
                    println!("  - Function: {}", name);
                }
            }
        }
    }
    
    // Find references
    if let Ok(refs) = analysis.references(&position) {
        println!("\nReferences found:");
        for reference in refs {
            println!("  - {:?}", reference);
        }
    }
    
    Ok(())
}
