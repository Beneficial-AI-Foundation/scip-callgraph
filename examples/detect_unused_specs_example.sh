#!/bin/bash
# Example: How to use the detect_unused_specs tool

# Navigate to your project directory
# cd /path/to/your/verus/project

# Option 1: Auto-generate SCIP index and detect unused specs
echo "Running detection with auto-generated SCIP index..."
cargo run -p metrics-cli --bin detect_unused_specs -- .

# Option 2: Use existing SCIP index (faster for repeated runs)
echo ""
echo "Running detection with existing SCIP index..."
cargo run -p metrics-cli --bin detect_unused_specs -- . ./project_index_scip.json

# The tool will create:
# - <project_name>_index_scip.json (if not provided)
# - <project_name>_atoms.json
# - <project_name>_unused_specs.json

# You can also use it on other projects:
# cargo run -p metrics-cli --bin detect_unused_specs -- /path/to/other/project

echo ""
echo "Results saved to <project_name>_unused_specs.json"
echo ""
echo "Parse the JSON results:"
echo "  jq '.summary' <project_name>_unused_specs.json"
echo "  jq '.unused_specs[] | .display_name' <project_name>_unused_specs.json"
echo ""
echo "Or use the provided parser script:"
echo "  ./examples/parse_unused_specs_json.sh <project_name>_unused_specs.json"

