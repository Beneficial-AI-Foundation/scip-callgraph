use std::collections::HashMap;
use std::fs::File;
use std::io::Write;

pub struct FunctionNode {
    symbol: String,
    display_name: String,
    body: Option<String>,
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub fn generate_call_graph_svg(
    call_graph: &HashMap<String, FunctionNode>,
    output_path: &str,
) -> std::io::Result<()> {
    let mut svg = String::new();
    let node_radius = 20;
    let positions = calculate_positions(call_graph);

    svg.push_str("<svg xmlns='http://www.w3.org/2000/svg' version='1.1'>\n");

    for node in call_graph.values() {
        let (x, y) = positions[&node.symbol];
        let body = node
            .body
            .as_ref()
            .map(|b| escape_html(b))
            .unwrap_or_default();
        svg.push_str(&format!(
            "<g>\
                <circle cx='{x}' cy='{y}' r='{r}' fill='#4a90e2' stroke='#222' stroke-width='2'/>\
                <text x='{x}' y='{y}' text-anchor='middle' alignment-baseline='middle' fill='#fff' font-size='14'>{label}</text>\
                <title>{body}</title>\
            </g>\n",
            x=x, y=y, r=node_radius, label=escape_html(&node.display_name), body=body
        ));
    }

    svg.push_str("</svg>\n");

    let mut file = File::create(output_path)?;
    file.write_all(svg.as_bytes())?;
    Ok(())
}

fn calculate_positions(call_graph: &HashMap<String, FunctionNode>) -> HashMap<String, (i32, i32)> {
    // Dummy implementation for positions calculation
    let mut positions = HashMap::new();
    let mut x = 50;
    let mut y = 50;
    for key in call_graph.keys() {
        positions.insert(key.clone(), (x, y));
        x += 100;
        y += 100;
    }
    positions
}
