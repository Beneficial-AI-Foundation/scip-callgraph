#!/bin/bash
# Quick start script for the interactive call graph viewer

set -e

echo "🚀 SCIP Call Graph Interactive Viewer - Quick Start"
echo "=================================================="
echo ""

# Check if an argument was provided
if [ -n "$1" ]; then
    INPUT_ARG="$1"
    
    # Check if it's a directory (project path) or a file (SCIP JSON)
    if [ -d "$INPUT_ARG" ]; then
        # It's a directory - generate SCIP index
        PROJECT_PATH="$INPUT_ARG"
        echo "📁 Project path provided: $PROJECT_PATH"
        
        echo ""
        echo "Step 1: Generating SCIP index for project..."
        echo "============================================="
        
        # Build the generate_index_scip_json binary if needed
        if [ ! -f "target/release/generate_index_scip_json" ]; then
            echo "Building generate_index_scip_json binary..."
            cargo build --release -p metrics-cli --bin generate_index_scip_json
        fi
        
        # Generate SCIP JSON from project
        echo "Running: generate_index_scip_json on $PROJECT_PATH"
        echo "(This may take a while for large projects...)"
        
        # Capture output and extract the file path
        # The output format is: "Done! Output file: <path>"
        OUTPUT_FILE=$(./target/release/generate_index_scip_json "$PROJECT_PATH" 2>&1 | tee /dev/tty | grep -o "Output file:.*" | sed 's/Output file: //')
        
        if [ -z "$OUTPUT_FILE" ]; then
            echo ""
            echo "❌ Failed to extract output file path from generate_index_scip_json"
            echo "The command may have failed or output format changed."
            exit 1
        fi
        
        # Verify the file exists
        if [ ! -f "$OUTPUT_FILE" ]; then
            echo ""
            echo "❌ SCIP JSON file not found at: $OUTPUT_FILE"
            exit 1
        fi
        
        SCIP_FILE="$OUTPUT_FILE"
        echo ""
        echo "✓ Generated SCIP file: $SCIP_FILE"
        SKIP_EXPORT=false
        
    elif [ -f "$INPUT_ARG" ]; then
        # It's a file - determine if it's SCIP JSON or D3 JSON
        echo "📄 JSON file provided: $INPUT_ARG"
        
        # Check if it's a D3 JSON (has "nodes" and "links" fields)
        if grep -q '"nodes"' "$INPUT_ARG" && grep -q '"links"' "$INPUT_ARG"; then
            # It's a D3 JSON file - skip SCIP generation and export
            echo "✓ Detected D3 graph JSON (already exported format)"
            echo "✓ Skipping SCIP generation and export steps"
            
            # Copy directly to the final location
            cp "$INPUT_ARG" call_graph_d3.json
            SKIP_EXPORT=true
        else
            # It's a SCIP JSON file
            echo "✓ Detected SCIP JSON file"
            SCIP_FILE="$INPUT_ARG"
            SKIP_EXPORT=false
        fi
        
    else
        echo "❌ Error: '$INPUT_ARG' is neither a directory nor a file"
        echo ""
        echo "Usage: $0 [project_path_or_scip_json]"
        echo ""
        echo "Examples:"
        echo "  $0 /path/to/rust/project              # Generate SCIP from project"
        echo "  $0 /path/to/index_scip.json           # Use existing SCIP JSON"
        echo "  $0                                    # Look for SCIP JSON in current dir"
        exit 1
    fi
else
    # No argument provided - look for existing SCIP JSON files
    echo "ℹ️  No project path provided. Looking for existing SCIP JSON files..."
    echo ""
    
    if [ -f "examples/example_data/index_scip_libsignal_deps.json" ]; then
        SCIP_FILE="examples/example_data/index_scip_libsignal_deps.json"
        echo "✓ Found example SCIP file: $SCIP_FILE"
        SKIP_EXPORT=false
    elif [ -f "index_scip.json" ]; then
        SCIP_FILE="index_scip.json"
        echo "✓ Found SCIP file: $SCIP_FILE"
        SKIP_EXPORT=false
    else
        echo "❌ No SCIP JSON file found."
        echo ""
        echo "Usage: $0 [project_path_or_scip_json]"
        echo ""
        echo "Options:"
        echo "  1. Provide a Rust project directory (auto-generates SCIP):"
        echo "     $0 /path/to/rust/project"
        echo ""
        echo "  2. Provide an existing SCIP JSON file:"
        echo "     $0 /path/to/index_scip.json"
        echo ""
        echo "  3. Provide an existing D3 graph JSON file (fastest):"
        echo "     $0 /path/to/call_graph_d3.json"
        echo ""
        echo "  4. Or place index_scip.json in the current directory and run:"
        echo "     $0"
        echo ""
        echo "To manually generate SCIP data:"
        echo "  1. Run: rust-analyzer scip ."
        echo "  2. Run: scip print --json index.scip > index_scip.json"
        exit 1
    fi
fi

# Only build and export if we don't already have D3 JSON
if [ "$SKIP_EXPORT" = false ]; then
    echo ""
    echo "Step 2: Building Rust export binary..."
    echo "======================================="
    cargo build --release -p metrics-cli --bin export_call_graph_d3

    echo ""
    echo "Step 3: Exporting call graph to D3 format..."
    echo "============================================="
    ./target/release/export_call_graph_d3 "$SCIP_FILE" -o call_graph_d3.json

    if [ ! -f "call_graph_d3.json" ]; then
        echo "❌ Failed to generate call_graph_d3.json"
        exit 1
    fi
else
    echo ""
    echo "Step 2-3: Skipped (using existing D3 JSON)"
    echo "==========================================="
fi

echo ""
echo "✓ Generated: call_graph_d3.json"
echo ""

# Get stats from JSON
NODES=$(grep -o '"id"' call_graph_d3.json | wc -l)
LINKS=$(grep -o '"source"' call_graph_d3.json | wc -l)
echo "📊 Graph Statistics:"
echo "   Nodes: $NODES"
echo "   Edges: $LINKS"

# Store absolute path before changing directory
GRAPH_JSON_PATH=$(realpath call_graph_d3.json)

echo ""
echo "Step 4: Setting up web viewer..."
echo "================================"

# Copy the JSON to the web/public directory for automatic loading
mkdir -p web/public
cp call_graph_d3.json web/public/graph.json
echo "✓ Copied graph data to web/public/graph.json for automatic loading"

cd web

if [ ! -d "node_modules" ]; then
    echo "Installing npm dependencies..."
    npm install
else
    echo "✓ Dependencies already installed"
fi

echo ""
echo "✅ Setup complete!"
echo ""
echo "📋 What's happening next:"
echo "   1. The development server will start"
echo "   2. Your browser will open automatically"
echo "   3. The graph will load automatically!"
echo "   4. If auto-load fails, manually load: $GRAPH_JSON_PATH"
echo ""
echo "🎉 You can now start exploring the call graph!"
echo ""
echo "🎛️ Features to try:"
echo "   - Search for functions by name"
echo "   - Click nodes to see callers/callees"
echo "   - Adjust depth slider for focused views"
echo "   - Filter by source type (libsignal/external)"
echo ""
echo "Starting development server..."
echo "=============================="

npm run dev

