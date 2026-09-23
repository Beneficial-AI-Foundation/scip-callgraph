# Spec Halstead Metrics - How It Works

## Overview

`crates/verus-metrics/src/spec_halstead.rs` computes Halstead complexity
metrics for Verus specification clauses by parsing each clause as an
expression with `verus_syn` and counting operators and operands in the AST.

---

## Architecture

```
Spec Text → is_prose check → Preprocess → Parse (verus_syn) → Visit AST → Compute Metrics
```

### Pipeline stages

**1. Prose rejection** (`is_prose`, see below)

Clauses that read as natural-language documentation are rejected with an
error rather than scored; scoring English sentences as code would inflate
operand counts with meaningless words.

**2. Preprocessing** (`preprocess_verus_spec`)

Minimal, because `verus_syn` parses Verus syntax natively: strip `//`
comments (they interfere with parsing) and skip empty or incomplete
fragments (`(`, trailing `(`). There is no rewriting of Verus constructs —
`forall|`, `exists|`, `==>`, `old()`, `@`, `&&&`, `=~=`, `#[trigger]`,
chained comparisons, and `as int`/`as nat` casts all go straight to the
parser. (Earlier versions used plain `syn` and rewrote these; that code was
removed in the `verus_syn` migration.)

**3. Parsing**
```rust
let expr: Expr = verus_syn::parse_str(&preprocessed)?;
```

**4. AST traversal** (`HalsteadVisitor`, implementing `verus_syn::visit::Visit`)

Walks the AST, categorizing each node as operator or operand.

**5. Metric calculation**
```
length     = N1 + N2              // total operators + operands
vocabulary = n1 + n2              // unique operators + operands
difficulty = (n1/2) × (N2/n2)
volume     = length × log2(vocabulary)
effort     = difficulty × volume
```

The result struct `SpecHalsteadMetrics` carries all five plus the raw
counts: `n1_unique_operators`, `n2_unique_operands`, `n1_total_operators`,
`n2_total_operands`.

---

## Token Classification

### Operators (things that DO)
- Binary: `+`, `-`, `*`, `==`, `<`, `&&`, etc.
- Unary: `!`, `-`, `*` (deref)
- Structural: `.` (field access), `[]` (index), `()` (parentheses), `&` (ref)
- Calls: function calls count a `call` operator; method calls count the
  method name as the operator
- Keywords: `as` (cast)

### Operands (things that ARE)
- Variables and paths: `x`, `result`, `i32::MAX`
- Literals: `10`, `"text"`, `true`
- Constants: `FIELD_MODULUS`
- Field names: `0` (in `x.0`)
- Function names: `f` in `f(x)` (the call itself is the operator)

Function calls are counted syntactically — the callee's body is never
expanded. `f(g(x)) == y` scores 3 operators (`call`, `call`, `==`) and
4 operands (`f`, `g`, `x`, `y`), regardless of what `f` and `g` do.

---

## Example Walkthrough

### Input spec
```rust
x.0 < FIELD_MODULUS && y.0 < FIELD_MODULUS
```

### AST structure (simplified)
```
BinaryExpr(&&)
├─ left: BinaryExpr(<)
│  ├─ left: FieldExpr(.)
│  │  ├─ base: Path(x)
│  │  └─ field: 0
│  └─ right: Path(FIELD_MODULUS)
└─ right: BinaryExpr(<)
   ├─ left: FieldExpr(.)
   │  ├─ base: Path(y)
   │  └─ field: 0
   └─ right: Path(FIELD_MODULUS)
```

### Token counting

- Operators: `&&`, `<`, `.`, `<`, `.` — 5 total, 3 unique
- Operands: `x`, `0`, `FIELD_MODULUS`, `y`, `0`, `FIELD_MODULUS` — 6 total, 4 unique

### Metrics
```
n1 = 3, n2 = 4, N1 = 5, N2 = 6

halstead_length = 5 + 6 = 11
vocabulary      = 3 + 4 = 7
difficulty      = (3/2) × (6/4) = 2.25
volume          = 11 × log2(7) = 30.88
effort          = 2.25 × 30.88 = 69.48
```

---

## Prose Detection (`is_prose`)

The single most surprising behaviour for a newcomer: some "specs" get
*rejected*. Spec extraction occasionally captures doc comments or English
sentences that sit where a clause was expected; measured on curve25519-dalek,
roughly 10% of extracted clauses were prose, not specifications. Scoring them
would corrupt the metrics, so `analyze_spec` returns an error for them and
callers skip the clause.

The heuristic (in order):
1. strings shorter than 10 chars are never prose;
2. `///` or `//!` prefixes are prose;
3. a list of English phrase indicators (`"However,"`, `"i.e."`,
   `"must be clear"`, `"is equivalent to"`, …) anywhere in the string;
4. English starter words (`"The "`, `"We "`, `"Given"`, …) at the start,
   unless comparison operators (`==`, `!=`, `<=`, `>=`) appear;
5. a dangling `*/` (comment fragment);
6. ratio test: more than 50 letters but fewer than 3 operator characters.

`is_prose` is exported (`verus_metrics::is_prose`) so callers can
distinguish "prose, skip silently" from real parse failures.

---

## Error Handling

```rust
pub fn analyze_spec(spec_text: &str) -> Result<SpecHalsteadMetrics, String>
```

- Empty string → `Ok` with all-zero metrics
- Prose (per `is_prose`) → `Err("Skipped prose/documentation: ...")`
- Empty after preprocessing (e.g. a bare `decreases` clause) →
  `Err("Skipped non-expression clause: ...")`
- Unparseable → `Err` with the `verus_syn` parse message

---

## Usage

```rust
use verus_metrics::{analyze_spec, is_prose, SpecHalsteadMetrics};

let spec = "x < FIELD_MODULUS && y < FIELD_MODULUS";
let metrics = analyze_spec(spec)?;
println!("Length: {}", metrics.halstead_length);
```

Demo:
```bash
cargo run -p metrics-cli --bin demo_spec_halstead
```

Tests (21, including the prose-detection and chained-comparison suites):
```bash
cargo test -p verus-metrics spec_halstead
```

---

## Design Decisions

### Why AST parsing over regex?

Regex token matching miscounts inside strings and comments and cannot
handle nesting. The AST respects both, and adding a new expression type is
one more match arm.

### Why `verus_syn` over `syn` + preprocessing?

The first version used `syn 2.0` and rewrote Verus constructs into
parseable Rust (`forall|x|` → `|x|`, `==>` → `||`, stripped `old()`).
Those rewrites were lossy approximations and a steady source of parse
failures. `verus_syn` is the Verus project's own fork of `syn` and parses
the real syntax, so the rewrites were deleted. The dependency is pinned in
the workspace `Cargo.toml` (`verus_syn = "0.0.0-2025-11-16-0050"`, features
`full`, `visit`, `parsing`).

### Why the visitor pattern?

`verus_syn::visit::Visit` gives automatic traversal for every expression
type we don't explicitly match, so unhandled node kinds degrade to "visit
children" instead of being dropped.

---

## Limitations

1. **Syntactic only**: function calls are one operator plus a name operand;
   bodies are never expanded (see `test_function_calls_are_syntactic_only`).
2. **Prose heuristic**: pattern-based; unusual phrasing can slip through or
   be over-flagged.
3. **Type complexity**: types appearing in specs are not scored separately.
4. **Complex chained comparisons** with arithmetic in the middle expression
   (`a <= b - c < d`) are known edge cases.
