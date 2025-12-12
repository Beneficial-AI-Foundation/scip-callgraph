#!/usr/bin/env python3
"""
Add verification status to nodes in graph.json based on verification_results.json.

Status mapping:
- "verified" (green): function found in verified_functions
- "failed" (red): function found in failed_functions  
- "unverified" (grey): function found in unverified_functions
- null/not present (blue): function not found in verification results
"""

import json
import argparse
from pathlib import Path
from typing import Optional


def normalize_path(path: str) -> str:
    """
    Normalize a file path for comparison.
    
    Handles cases like:
    - verification_results.json: curve25519-dalek/curve25519-dalek/src/lemmas/...
    - graph.json: curve25519-dalek/src/lemmas/...
    
    We normalize to just the part starting from 'src/' to avoid
    issues with duplicate directory prefixes.
    """
    # Remove common prefixes and normalize
    path = path.replace("file://", "")
    
    # Find 'src/' as the most reliable anchor point
    # This handles cases where project name appears multiple times in path
    if "/src/" in path:
        # Extract everything from the last occurrence of a known root dir before src
        # e.g., "curve25519-dalek/curve25519-dalek/src/..." -> "src/..."
        src_idx = path.find("/src/")
        return path[src_idx + 1:]  # Return "src/..."
    
    # Fallback: try to find common project directories
    parts = path.split("/")
    for i, part in enumerate(parts):
        if part == "src":
            return "/".join(parts[i:])
    
    return path


def build_verification_lookup(verification_data: dict) -> dict:
    """
    Build a lookup dictionary from verification results.
    
    Returns a dict mapping (display_name, normalized_path) -> status
    Also creates a secondary index by just display_name for fallback matching.
    """
    lookup = {}
    by_name = {}  # Secondary index: display_name -> [(path, status), ...]
    
    verification = verification_data.get("verification", {})
    
    status_mapping = {
        "verified_functions": "verified",
        "failed_functions": "failed",
        "unverified_functions": "unverified",
    }
    
    for key, status in status_mapping.items():
        functions = verification.get(key, [])
        for func in functions:
            display_name = func.get("display-name", "")
            code_path = func.get("code-path", "")
            lines_start = func.get("code-text", {}).get("lines-start")
            lines_end = func.get("code-text", {}).get("lines-end")
            
            normalized_path = normalize_path(code_path)
            
            # Primary key: (name, path, start_line)
            primary_key = (display_name, normalized_path, lines_start)
            lookup[primary_key] = status
            
            # Secondary index by name + path (without line numbers)
            secondary_key = (display_name, normalized_path)
            if secondary_key not in lookup:
                lookup[secondary_key] = status
            
            # Tertiary index by just name
            if display_name not in by_name:
                by_name[display_name] = []
            by_name[display_name].append({
                "path": normalized_path,
                "status": status,
                "lines_start": lines_start,
                "lines_end": lines_end,
            })
    
    return lookup, by_name


def find_verification_status(
    node: dict,
    lookup: dict,
    by_name: dict
) -> Optional[str]:
    """
    Find the verification status for a given node.
    
    Tries multiple matching strategies:
    1. Exact match on (display_name, path, start_line)
    2. Match on (display_name, path)
    3. Match on display_name alone (if unique or all have same status)
    """
    display_name = node.get("display_name", "")
    
    # Normalize the node's path
    full_path = node.get("full_path", "")
    relative_path = node.get("relative_path", "")
    
    # Try with relative_path first, then full_path
    for path in [relative_path, full_path]:
        if not path:
            continue
            
        normalized_path = normalize_path(path)
        start_line = node.get("start_line")
        
        # Strategy 1: Exact match with line number
        if start_line is not None:
            key1 = (display_name, normalized_path, start_line)
            if key1 in lookup:
                return lookup[key1]
        
        # Strategy 2: Match without line number
        key2 = (display_name, normalized_path)
        if key2 in lookup:
            return lookup[key2]
        
        # Try partial path matching
        for (name, lpath, *rest), status in lookup.items():
            if name == display_name:
                # Check if paths overlap
                if lpath in normalized_path or normalized_path in lpath:
                    # Additional check: if we have line numbers, verify they match
                    if rest and start_line is not None:
                        lookup_start = rest[0]
                        if lookup_start == start_line:
                            return status
                    elif not rest:
                        return status
    
    # Strategy 3: Fallback to name-only matching
    if display_name in by_name:
        matches = by_name[display_name]
        # If all matches have the same status, use it
        statuses = set(m["status"] for m in matches)
        if len(statuses) == 1:
            return matches[0]["status"]
        
        # If there's only one match, use it
        if len(matches) == 1:
            return matches[0]["status"]
    
    return None


def process_graph(graph_path: Path, verification_path: Path, output_path: Path):
    """Process the graph and add verification status to each node."""
    
    print(f"Reading graph from: {graph_path}")
    with open(graph_path, "r") as f:
        graph_data = json.load(f)
    
    print(f"Reading verification results from: {verification_path}")
    with open(verification_path, "r") as f:
        verification_data = json.load(f)
    
    # Build lookup tables
    lookup, by_name = build_verification_lookup(verification_data)
    
    print(f"Built lookup with {len(lookup)} entries")
    print(f"Functions by name: {len(by_name)} unique names")
    
    # Process each node
    nodes = graph_data.get("nodes", [])
    stats = {"verified": 0, "failed": 0, "unverified": 0, "unknown": 0}
    
    for node in nodes:
        status = find_verification_status(node, lookup, by_name)
        if status:
            node["verification_status"] = status
            stats[status] += 1
        else:
            stats["unknown"] += 1
            # Don't add the field if no status found
    
    print("\nVerification status statistics:")
    print(f"  Verified (green):   {stats['verified']}")
    print(f"  Failed (red):       {stats['failed']}")
    print(f"  Unverified (grey):  {stats['unverified']}")
    print(f"  Unknown (blue):     {stats['unknown']}")
    print(f"  Total nodes:        {len(nodes)}")
    
    # Write output
    print(f"\nWriting enriched graph to: {output_path}")
    with open(output_path, "w") as f:
        json.dump(graph_data, f, indent=2)
    
    print("Done!")


def main():
    parser = argparse.ArgumentParser(
        description="Add verification status to graph.json nodes"
    )
    parser.add_argument(
        "--graph",
        type=Path,
        default=Path("web/public/graph.json"),
        help="Path to graph.json (default: web/public/graph.json)"
    )
    parser.add_argument(
        "--verification",
        type=Path,
        default=Path("data/verification_results.json"),
        help="Path to verification_results.json (default: data/verification_results.json)"
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=None,
        help="Output path (default: overwrite input graph.json)"
    )
    
    args = parser.parse_args()
    
    output_path = args.output or args.graph
    
    process_graph(args.graph, args.verification, output_path)


if __name__ == "__main__":
    main()
