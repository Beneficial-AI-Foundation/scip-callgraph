
# Metrics Correlation Analysis Summary

## Dataset Overview
- Total functions: 212
- Functions with proofs: 56
- Non-trivial proofs: 35
- Functions with RCA metrics: 170

## Key Correlations (Non-trivial proofs only)

| Code Length → Proof Effort | r = 0.353 | n = 29 |
| Cyclomatic → Lemmas Count | r = 0.023 | n = 23 |
| Spec Effort → Proof Effort | r = -0.265 | n = 28 |
| Spec Length → Direct Lemmas | r = 0.151 | n = 22 |

## Insights

- **Weak/no correlation** (0.02): Cyclomatic → Lemmas Count - interesting!
- **Weak/no correlation** (0.15): Spec Length → Direct Lemmas - interesting!

## Top 5 Functions by Proof Overhead

| Function | Proof Overhead Ratio |
|----------|---------------------|
| FieldElement::is_negative | 41830.5x |
| FieldElement::is_zero | 31346.3x |
| is_canonical | 3490.8x |
| reduce | 852.3x |
| FieldElement51::from_bytes | 476.5x |
