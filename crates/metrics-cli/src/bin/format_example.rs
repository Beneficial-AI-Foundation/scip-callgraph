use prettyplease::unparse;
use syn::{parse_file, File};

fn main() {
    // Parse a complete Rust file (not just a block)
    let code = r#"
fn main() {
    let x = 1;
    println!("x = {}", x);
}
"#;
    let syntax_tree: File = parse_file(code).unwrap();
    let formatted = unparse(&syntax_tree);
    println!("{}", formatted);
}
