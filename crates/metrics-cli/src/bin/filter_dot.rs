use clap::Parser;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};

/// Filter DOT files to remove core:: entries
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Input DOT file to filter
    input_dot_file: String,

    /// Output DOT file (optional, prints to stdout if not provided)
    output_dot_file: Option<String>,
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    // Open and read the input file
    let file = File::open(&args.input_dot_file)?;
    let reader = BufReader::new(file);

    // Prepare the output
    let mut output: Vec<String> = Vec::new();

    // Keep track of whether we're in digraph definition
    let mut in_digraph = false;

    // Process each line
    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();

        // Add non-node lines directly
        if trimmed.starts_with("digraph") {
            in_digraph = true;
            output.push(line.clone());
        } else if trimmed == "}" && in_digraph {
            in_digraph = false;
            output.push(line.clone());
        } else if in_digraph {
            // Filter out nodes that start with "core::"
            if !trimmed.contains("\"core::") {
                // Check for edges with core::
                if trimmed.contains("->") {
                    let parts: Vec<&str> = trimmed.split("->").collect();
                    if parts.len() == 2 {
                        let source = parts[0].trim();
                        let target = parts[1].split_whitespace().next().unwrap_or("");

                        // Skip edges where either source or target starts with "core::"
                        if !source.contains("\"core::") && !target.contains("\"core::") {
                            output.push(line.clone());
                        }
                    } else {
                        output.push(line.clone());
                    }
                } else {
                    output.push(line.clone());
                }
            }
        } else {
            output.push(line.clone());
        }
    }

    // Write the output
    if let Some(path) = &args.output_dot_file {
        let mut output_file = File::create(path)?;
        for line in output {
            writeln!(output_file, "{}", line)?;
        }
        println!("Filtered DOT file written to: {}", path);
    } else {
        for line in output {
            println!("{}", line);
        }
    }

    Ok(())
}
