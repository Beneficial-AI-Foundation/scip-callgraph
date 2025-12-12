#!/usr/bin/env python3
"""
Enrich a call graph JSON with similar lemmas for each node.

This script uses verus_lemma_finder to find semantically similar lemmas
for each function/lemma in the call graph and adds them to the graph JSON.

Usage (recommended - uses uv):
    uv run python scripts/enrich_graph_with_similar_lemmas.py \
        --graph web/public/graph.json \
        --index /path/to/lemma_index.json \
        --output web/public/graph_enriched.json \
        --top-k 3

Setup (first time only):
    uv sync --extra enrich

Example workflow:
    1. Generate call graph with Rust:
       cargo run --bin scip-to-call-graph -- input.scip -o graph.json
    
    2. Build lemma index (in verus_lemma_finder):
       cd /path/to/verus_lemma_finder
       uv run python -m verus_lemma_finder index scip_data.json -o lemma_index.json
    
    3. Enrich graph with similar lemmas:
       cd /path/to/scip-callgraph
       uv run python scripts/enrich_graph_with_similar_lemmas.py \
           --graph graph.json \
           --index /path/to/lemma_index.json
"""

import argparse
import json
import sys
from pathlib import Path


def enrich_graph(
    graph_path: Path,
    index_path: Path,
    output_path: Path | None = None,
    top_k: int = 3,
    verbose: bool = True,
) -> dict:
    """
    Enrich a call graph with similar lemmas for each node.
    
    Args:
        graph_path: Path to the input call graph JSON
        index_path: Path to the lemma index JSON
        output_path: Path to write the enriched graph (default: overwrite input)
        top_k: Number of similar lemmas per node
        verbose: Whether to print progress
    
    Returns:
        The enriched graph dictionary
    """
    # Import verus_lemma_finder
    try:
        from verus_lemma_finder import get_similar_lemmas_dict, load_searcher
    except ImportError:
        print("❌ Error: verus_lemma_finder not found.")
        print()
        print("Install it with one of:")
        print("  pip install /path/to/verus_lemma_finder")
        print("  pip install git+https://github.com/Beneficial-AI-Foundation/verus_lemma_finder.git")
        print()
        print("Or add it to PYTHONPATH:")
        print("  export PYTHONPATH=/path/to/verus_lemma_finder/src:$PYTHONPATH")
        sys.exit(1)
    
    if output_path is None:
        output_path = graph_path
    
    # Load the graph
    if verbose:
        print(f"📂 Loading graph from: {graph_path}")
    
    with open(graph_path) as f:
        graph = json.load(f)
    
    nodes = graph.get("nodes", [])
    if verbose:
        print(f"   Found {len(nodes)} nodes")
    
    # Load the lemma searcher (once, for efficiency)
    if verbose:
        print(f"📂 Loading lemma index from: {index_path}")
    
    searcher = load_searcher(index_path, use_embeddings=True)
    
    if verbose:
        print()
        print(f"🔍 Finding top {top_k} similar lemmas for each node...")
        print()
    
    # Process each node
    enriched_count = 0
    for i, node in enumerate(nodes):
        # Use display_name as the primary query
        display_name = node.get("display_name", "")
        if not display_name:
            continue
        
        # Build a rich query using name + body snippet for better matching
        body = node.get("body", "")
        if body:
            # Use first few lines of body for context
            body_lines = body.split('\n')[:5]
            query = f"{display_name} {' '.join(body_lines)}"
        else:
            query = display_name
        
        # Get similar lemmas
        similar = get_similar_lemmas_dict(
            query=query,
            searcher=searcher,
            top_k=top_k,
            exclude_self=True,
        )
        
        # Add to node if we found any
        if similar:
            node["similar_lemmas"] = similar
            enriched_count += 1
        
        # Progress indicator
        if verbose and ((i + 1) % 100 == 0 or i + 1 == len(nodes)):
            print(f"   Processed {i + 1}/{len(nodes)} nodes...", end='\r')
    
    if verbose:
        print()
        print()
    
    # Save the enriched graph
    if verbose:
        print(f"💾 Saving enriched graph to: {output_path}")
    
    with open(output_path, 'w') as f:
        json.dump(graph, f, indent=2)
    
    if verbose:
        print()
        print("=" * 60)
        print("✅ Graph enrichment complete!")
        print("=" * 60)
        print(f"   Nodes processed: {len(nodes)}")
        print(f"   Nodes with similar lemmas: {enriched_count}")
        print(f"   Output: {output_path}")
    
    return graph


def main():
    parser = argparse.ArgumentParser(
        description="Enrich a call graph JSON with similar lemmas for each node.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Example:
    python scripts/enrich_graph_with_similar_lemmas.py \\
        --graph web/public/graph.json \\
        --index /path/to/lemma_index.json \\
        --top-k 3
        """
    )
    
    parser.add_argument(
        "--graph", "-g",
        type=Path,
        required=True,
        help="Path to the call graph JSON file"
    )
    
    parser.add_argument(
        "--index", "-i",
        type=Path,
        required=True,
        help="Path to the lemma index JSON file"
    )
    
    parser.add_argument(
        "--output", "-o",
        type=Path,
        default=None,
        help="Output file path (default: overwrite input graph)"
    )
    
    parser.add_argument(
        "--top-k", "-k",
        type=int,
        default=3,
        help="Number of similar lemmas per node (default: 3)"
    )
    
    parser.add_argument(
        "--quiet", "-q",
        action="store_true",
        help="Suppress progress output"
    )
    
    args = parser.parse_args()
    
    # Validate inputs
    if not args.graph.exists():
        print(f"❌ Error: Graph file not found: {args.graph}")
        sys.exit(1)
    
    if not args.index.exists():
        print(f"❌ Error: Index file not found: {args.index}")
        sys.exit(1)
    
    # Run enrichment
    enrich_graph(
        graph_path=args.graph,
        index_path=args.index,
        output_path=args.output,
        top_k=args.top_k,
        verbose=not args.quiet,
    )


if __name__ == "__main__":
    main()

