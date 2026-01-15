#!/usr/bin/env python3
"""
Find curve25519-dalek functions that are called by libsignal.
Outputs a map with GitHub links to the libsignal callers.

Usage:
    python scripts/find_dalek_called_by_signal.py          # text output
    python scripts/find_dalek_called_by_signal.py json     # JSON output
    python scripts/find_dalek_called_by_signal.py csv      # CSV output
    python scripts/find_dalek_called_by_signal.py md       # Markdown file
    python scripts/find_dalek_called_by_signal.py md out.md  # Custom filename
"""

import json
import csv
import sys
from collections import defaultdict
from datetime import datetime
from pathlib import Path

# Resolve paths relative to project root
SCRIPT_DIR = Path(__file__).parent
PROJECT_ROOT = SCRIPT_DIR.parent
DEFAULT_GRAPH_PATH = PROJECT_ROOT / 'web' / 'public' / 'graph.json'

# libsignal GitHub repo base URL (main branch)
LIBSIGNAL_GITHUB_BASE = "https://github.com/signalapp/libsignal/blob/main"

def make_github_link(relative_path: str, line: int) -> str:
    """Create a GitHub link to a specific file and line."""
    return f"{LIBSIGNAL_GITHUB_BASE}/{relative_path}#L{line}"

def get_dalek_functions_called_by_signal(graph_path=None):
    """Find all curve25519-dalek functions called by libsignal."""
    if graph_path is None:
        graph_path = DEFAULT_GRAPH_PATH
    
    with open(graph_path, 'r') as f:
        data = json.load(f)
    
    nodes = data['nodes']
    node_by_id = {n['id']: n for n in nodes}
    
    result_map = {}
    
    for node in nodes:
        node_id = node['id']
        
        if 'curve25519-dalek' not in node_id:
            continue
        
        dependents = node.get('dependents', [])
        
        for dep_id in dependents:
            if 'curve25519-dalek' not in dep_id:
                dep_node = node_by_id.get(dep_id, {})
                if dep_node.get('is_libsignal', False):
                    caller_path = dep_node.get('relative_path', '')
                    caller_line = dep_node.get('start_line', 0)
                    caller_name = dep_node.get('display_name', '')
                    
                    result_map[node_id] = {
                        'dalek_function': node.get('display_name', ''),
                        'dalek_path': node.get('relative_path', ''),
                        'caller_id': dep_id,
                        'caller_name': caller_name,
                        'caller_path': caller_path,
                        'caller_line': caller_line,
                        'github_link': make_github_link(caller_path, caller_line)
                    }
                    break
    
    return result_map

def output_markdown(result_map, output_file=None):
    """Generate a nicely formatted markdown file."""
    if output_file is None:
        output_file = PROJECT_ROOT / 'curve25519_dalek_functions_called_by_libsignal.md'
    
    # Group by dalek source file
    by_dalek_file = defaultdict(list)
    for dalek_id, info in result_map.items():
        by_dalek_file[info['dalek_path']].append((dalek_id, info))
    
    # Group by signal caller file  
    by_signal_file = defaultdict(list)
    for dalek_id, info in result_map.items():
        by_signal_file[info['caller_path']].append((dalek_id, info))
    
    lines = []
    lines.append("# curve25519-dalek Functions Called by libsignal")
    lines.append("")
    lines.append(f"> Generated on {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    lines.append("")
    lines.append(f"**Total:** {len(result_map)} curve25519-dalek functions are called by libsignal")
    lines.append("")
    
    # Summary table
    lines.append("## Summary")
    lines.append("")
    lines.append("| curve25519-dalek Source File | # Functions Used |")
    lines.append("|------------------------------|------------------|")
    for dalek_file in sorted(by_dalek_file.keys()):
        count = len(by_dalek_file[dalek_file])
        short_name = dalek_file.replace('curve25519-dalek/', '')
        lines.append(f"| `{short_name}` | {count} |")
    lines.append("")
    
    # Quick reference table
    lines.append("## Quick Reference")
    lines.append("")
    lines.append("| Function | curve25519-dalek Location | Example libsignal Caller |")
    lines.append("|----------|---------------------------|--------------------------|")
    for dalek_id, info in sorted(result_map.items(), key=lambda x: x[1]['dalek_function']):
        func = info['dalek_function']
        dalek_short = info['dalek_path'].replace('curve25519-dalek/', '')
        caller_link = f"[`{info['caller_name']}`]({info['github_link']})"
        lines.append(f"| `{func}` | `{dalek_short}` | {caller_link} |")
    lines.append("")
    
    # Detailed sections by dalek file
    lines.append("## Details by curve25519-dalek Source File")
    lines.append("")
    
    for dalek_file in sorted(by_dalek_file.keys()):
        short_name = dalek_file.replace('curve25519-dalek/', '')
        lines.append(f"### `{short_name}`")
        lines.append("")
        
        items = sorted(by_dalek_file[dalek_file], key=lambda x: x[1]['dalek_function'])
        for dalek_id, info in items:
            lines.append(f"#### `{info['dalek_function']}`")
            lines.append("")
            lines.append(f"- **Called by:** [`{info['caller_name']}`]({info['github_link']})")
            lines.append(f"- **Caller location:** `{info['caller_path']}:{info['caller_line']}`")
            lines.append(f"- **Full SCIP ID:** `{dalek_id}`")
            lines.append("")
    
    # Section by signal caller area
    lines.append("## Details by libsignal Caller Area")
    lines.append("")
    
    # Group callers by top-level directory
    by_signal_area = defaultdict(list)
    for dalek_id, info in result_map.items():
        # Extract area like "rust/zkgroup", "rust/core", etc.
        parts = info['caller_path'].split('/')
        if len(parts) >= 2:
            area = '/'.join(parts[:2])
        else:
            area = info['caller_path']
        by_signal_area[area].append((dalek_id, info))
    
    for area in sorted(by_signal_area.keys()):
        lines.append(f"### `{area}`")
        lines.append("")
        items = sorted(by_signal_area[area], key=lambda x: x[1]['dalek_function'])
        
        lines.append("| curve25519-dalek Function | Caller | Link |")
        lines.append("|---------------------------|--------|------|")
        for dalek_id, info in items:
            lines.append(f"| `{info['dalek_function']}` | `{info['caller_name']}` | [View]({info['github_link']}) |")
        lines.append("")
    
    content = '\n'.join(lines)
    
    with open(output_file, 'w') as f:
        f.write(content)
    
    print(f"Markdown file written to: {output_file}")
    return output_file

def main():
    result_map = get_dalek_functions_called_by_signal()
    
    output_format = sys.argv[1] if len(sys.argv) > 1 else 'text'
    output_file = sys.argv[2] if len(sys.argv) > 2 else None
    
    if output_format == 'json':
        print(json.dumps(result_map, indent=2))
    elif output_format == 'csv':
        writer = csv.writer(sys.stdout)
        writer.writerow(['dalek_function_id', 'dalek_function', 'dalek_path', 
                        'caller_id', 'caller_name', 'caller_path', 'caller_line', 'github_link'])
        for dalek_id, info in sorted(result_map.items(), key=lambda x: x[1]['dalek_function']):
            writer.writerow([
                dalek_id,
                info['dalek_function'],
                info['dalek_path'],
                info['caller_id'],
                info['caller_name'],
                info['caller_path'],
                info['caller_line'],
                info['github_link']
            ])
    elif output_format == 'md':
        output_markdown(result_map, output_file)
    else:  # text (default)
        print(f"Found {len(result_map)} curve25519-dalek functions called by libsignal:\n")
        
        for dalek_id, info in sorted(result_map.items(), key=lambda x: x[1]['dalek_function']):
            print(f"curve25519-dalek function: {info['dalek_function']}")
            print(f"  Dalek path: {info['dalek_path']}")
            print(f"  Dalek ID: {dalek_id}")
            print(f"  Called by: {info['caller_name']}")
            print(f"  Caller path: {info['caller_path']}:{info['caller_line']}")
            print(f"  GitHub: {info['github_link']}")
            print()

if __name__ == '__main__':
    main()

