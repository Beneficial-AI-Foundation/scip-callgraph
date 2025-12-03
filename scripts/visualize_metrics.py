#!/usr/bin/env python3
"""
Metrics Correlation Visualizer

Generates plots to explore correlations between code, spec, and proof metrics.

Usage (recommended - uses uv):
    uv run --extra viz python scripts/visualize_metrics.py data/csv/pipeline_FINAL.csv

Setup (first time only):
    uv sync --extra viz
"""

import sys
import pandas as pd
import numpy as np
import matplotlib.pyplot as plt
import seaborn as sns
from pathlib import Path

# Set style
plt.style.use('seaborn-v0_8-whitegrid')
sns.set_palette("husl")

def load_and_clean_data(csv_path: str) -> pd.DataFrame:
    """Load CSV and clean data for analysis."""
    df = pd.read_csv(csv_path)
    
    # Convert numeric columns (they might have empty strings)
    numeric_cols = [
        'cyclomatic', 'cognitive', 'halstead_difficulty', 'halstead_effort', 'halstead_length',
        'requires_halstead_length', 'requires_halstead_difficulty', 'requires_halstead_effort',
        'ensures_halstead_length', 'ensures_halstead_difficulty', 'ensures_halstead_effort',
        'decreases_count',
        'direct_proof_length', 'direct_proof_difficulty', 'direct_proof_effort',
        'transitive_proof_length', 'transitive_proof_difficulty', 'transitive_proof_effort',
        'proof_depth', 'direct_lemmas_count', 'transitive_lemmas_count'
    ]
    
    for col in numeric_cols:
        if col in df.columns:
            df[col] = pd.to_numeric(df[col], errors='coerce')
    
    # Create derived metrics
    df['has_proof_bool'] = df['has_proof'] == 'yes'
    df['trivial_proof_bool'] = df['trivial_proof'] == 'yes'
    df['non_trivial_proof'] = (df['has_proof'] == 'yes') & (df['trivial_proof'] != 'yes')
    
    # Proof overhead ratio (avoid division by zero)
    df['proof_overhead_ratio'] = np.where(
        df['halstead_effort'] > 0,
        df['transitive_proof_effort'] / df['halstead_effort'],
        np.nan
    )
    
    # Spec to code ratio
    df['spec_to_code_ratio'] = np.where(
        df['halstead_length'] > 0,
        df['ensures_halstead_length'] / df['halstead_length'],
        np.nan
    )
    
    return df


def plot_code_vs_proof(df: pd.DataFrame, output_dir: Path):
    """Plot 1: Code complexity vs Proof effort."""
    fig, axes = plt.subplots(1, 2, figsize=(14, 6))
    
    # Filter to functions with proofs
    df_proofs = df[df['non_trivial_proof'] & df['transitive_proof_effort'].notna() & df['halstead_length'].notna()]
    
    if len(df_proofs) == 0:
        print("Warning: No data for code vs proof plot")
        return
    
    # Plot 1a: Code length vs Proof effort
    ax1 = axes[0]
    scatter = ax1.scatter(
        df_proofs['halstead_length'],
        df_proofs['transitive_proof_effort'],
        c=df_proofs['proof_depth'],
        cmap='viridis',
        alpha=0.7,
        s=50
    )
    ax1.set_xlabel('Code Halstead Length', fontsize=12)
    ax1.set_ylabel('Transitive Proof Effort', fontsize=12)
    ax1.set_title('Code Complexity vs Proof Effort', fontsize=14)
    ax1.set_yscale('log')
    plt.colorbar(scatter, ax=ax1, label='Proof Depth')
    
    # Add correlation coefficient
    corr = df_proofs[['halstead_length', 'transitive_proof_effort']].corr().iloc[0, 1]
    ax1.text(0.05, 0.95, f'r = {corr:.3f}', transform=ax1.transAxes, fontsize=11,
             verticalalignment='top', bbox=dict(boxstyle='round', facecolor='wheat', alpha=0.5))
    
    # Plot 1b: Cyclomatic vs Lemmas count
    df_cyclo = df_proofs[df_proofs['cyclomatic'].notna()]
    if len(df_cyclo) > 0:
        ax2 = axes[1]
        ax2.scatter(
            df_cyclo['cyclomatic'],
            df_cyclo['transitive_lemmas_count'],
            c=df_cyclo['cognitive'],
            cmap='plasma',
            alpha=0.7,
            s=50
        )
        ax2.set_xlabel('Cyclomatic Complexity', fontsize=12)
        ax2.set_ylabel('Transitive Lemmas Count', fontsize=12)
        ax2.set_title('Control Flow Complexity vs Proof Dependencies', fontsize=14)
        
        corr2 = df_cyclo[['cyclomatic', 'transitive_lemmas_count']].corr().iloc[0, 1]
        ax2.text(0.05, 0.95, f'r = {corr2:.3f}', transform=ax2.transAxes, fontsize=11,
                 verticalalignment='top', bbox=dict(boxstyle='round', facecolor='wheat', alpha=0.5))
    
    plt.tight_layout()
    plt.savefig(output_dir / 'plot1_code_vs_proof.png', dpi=150, bbox_inches='tight')
    plt.close()
    print("✓ Saved plot1_code_vs_proof.png")


def plot_spec_vs_proof(df: pd.DataFrame, output_dir: Path):
    """Plot 2: Spec complexity vs Proof effort."""
    fig, axes = plt.subplots(1, 2, figsize=(14, 6))
    
    # Filter to functions with specs and proofs
    df_spec = df[
        df['non_trivial_proof'] & 
        df['ensures_halstead_effort'].notna() & 
        df['transitive_proof_effort'].notna()
    ]
    
    if len(df_spec) == 0:
        print("Warning: No data for spec vs proof plot")
        return
    
    # Plot 2a: Ensures effort vs Proof effort
    ax1 = axes[0]
    scatter = ax1.scatter(
        df_spec['ensures_halstead_effort'],
        df_spec['transitive_proof_effort'],
        c=df_spec['proof_depth'],
        cmap='coolwarm',
        alpha=0.7,
        s=60
    )
    ax1.set_xlabel('Ensures Halstead Effort', fontsize=12)
    ax1.set_ylabel('Transitive Proof Effort', fontsize=12)
    ax1.set_title('Specification Complexity vs Proof Effort', fontsize=14)
    ax1.set_xscale('log')
    ax1.set_yscale('log')
    plt.colorbar(scatter, ax=ax1, label='Proof Depth')
    
    corr = df_spec[['ensures_halstead_effort', 'transitive_proof_effort']].corr().iloc[0, 1]
    ax1.text(0.05, 0.95, f'r = {corr:.3f}', transform=ax1.transAxes, fontsize=11,
             verticalalignment='top', bbox=dict(boxstyle='round', facecolor='wheat', alpha=0.5))
    
    # Plot 2b: Ensures length vs Direct lemmas
    ax2 = axes[1]
    ax2.scatter(
        df_spec['ensures_halstead_length'],
        df_spec['direct_lemmas_count'],
        alpha=0.7,
        s=60,
        c='steelblue'
    )
    ax2.set_xlabel('Ensures Halstead Length', fontsize=12)
    ax2.set_ylabel('Direct Lemmas Count', fontsize=12)
    ax2.set_title('Spec Size vs Direct Proof Dependencies', fontsize=14)
    
    corr2 = df_spec[['ensures_halstead_length', 'direct_lemmas_count']].corr().iloc[0, 1]
    ax2.text(0.05, 0.95, f'r = {corr2:.3f}', transform=ax2.transAxes, fontsize=11,
             verticalalignment='top', bbox=dict(boxstyle='round', facecolor='wheat', alpha=0.5))
    
    plt.tight_layout()
    plt.savefig(output_dir / 'plot2_spec_vs_proof.png', dpi=150, bbox_inches='tight')
    plt.close()
    print("✓ Saved plot2_spec_vs_proof.png")


def plot_correlation_heatmap(df: pd.DataFrame, output_dir: Path):
    """Plot 3: Correlation matrix heatmap."""
    # Select numeric columns for correlation
    cols = [
        'halstead_length', 'cyclomatic', 'cognitive',
        'ensures_halstead_length', 'ensures_halstead_effort',
        'direct_proof_effort', 'transitive_proof_effort',
        'proof_depth', 'transitive_lemmas_count'
    ]
    
    # Filter to available columns
    available_cols = [c for c in cols if c in df.columns]
    df_numeric = df[available_cols].dropna()
    
    if len(df_numeric) < 5:
        print("Warning: Not enough data for correlation heatmap")
        return
    
    # Compute correlation matrix
    corr_matrix = df_numeric.corr()
    
    # Create heatmap
    fig, ax = plt.subplots(figsize=(12, 10))
    
    # Create mask for upper triangle
    mask = np.triu(np.ones_like(corr_matrix, dtype=bool))
    
    sns.heatmap(
        corr_matrix,
        mask=mask,
        annot=True,
        fmt='.2f',
        cmap='RdBu_r',
        center=0,
        square=True,
        linewidths=0.5,
        cbar_kws={'shrink': 0.8},
        ax=ax
    )
    
    ax.set_title('Correlation Matrix: Code, Spec, and Proof Metrics', fontsize=14, pad=20)
    
    # Rotate labels
    plt.xticks(rotation=45, ha='right')
    plt.yticks(rotation=0)
    
    plt.tight_layout()
    plt.savefig(output_dir / 'plot3_correlation_heatmap.png', dpi=150, bbox_inches='tight')
    plt.close()
    print("✓ Saved plot3_correlation_heatmap.png")


def plot_proof_overhead(df: pd.DataFrame, output_dir: Path):
    """Plot 4: Top functions by proof overhead ratio."""
    # Filter and sort by proof overhead
    df_overhead = df[
        df['proof_overhead_ratio'].notna() & 
        (df['proof_overhead_ratio'] > 0) &
        df['non_trivial_proof']
    ].copy()
    
    if len(df_overhead) == 0:
        print("Warning: No data for proof overhead plot")
        return
    
    df_overhead = df_overhead.nlargest(15, 'proof_overhead_ratio')
    
    fig, ax = plt.subplots(figsize=(12, 8))
    
    # Create horizontal bar chart
    colors = plt.cm.Reds(np.linspace(0.3, 0.9, len(df_overhead)))
    bars = ax.barh(
        range(len(df_overhead)),
        df_overhead['proof_overhead_ratio'],
        color=colors
    )
    
    ax.set_yticks(range(len(df_overhead)))
    ax.set_yticklabels(df_overhead['function'], fontsize=10)
    ax.set_xlabel('Proof Overhead Ratio (Transitive Proof Effort / Code Effort)', fontsize=12)
    ax.set_title('Top 15 Functions by Proof Overhead', fontsize=14)
    
    # Add value labels
    for i, (bar, val) in enumerate(zip(bars, df_overhead['proof_overhead_ratio'])):
        ax.text(bar.get_width() + 0.5, bar.get_y() + bar.get_height()/2,
                f'{val:.1f}x', va='center', fontsize=9)
    
    ax.invert_yaxis()
    plt.tight_layout()
    plt.savefig(output_dir / 'plot4_proof_overhead.png', dpi=150, bbox_inches='tight')
    plt.close()
    print("✓ Saved plot4_proof_overhead.png")


def plot_proof_distribution(df: pd.DataFrame, output_dir: Path):
    """Plot 5: Distribution of proof effort by category."""
    fig, axes = plt.subplots(1, 2, figsize=(14, 6))
    
    # Plot 5a: Box plot of transitive proof effort
    df_proofs = df[df['transitive_proof_effort'].notna()].copy()
    df_proofs['category'] = df_proofs.apply(
        lambda x: 'Non-trivial Proof' if x['non_trivial_proof'] else (
            'Trivial Proof' if x['has_proof'] == 'yes' else 'No Proof'
        ), axis=1
    )
    
    ax1 = axes[0]
    categories = ['Non-trivial Proof', 'Trivial Proof']
    data_to_plot = [
        df_proofs[df_proofs['category'] == cat]['transitive_proof_effort'].dropna()
        for cat in categories
    ]
    data_to_plot = [d for d in data_to_plot if len(d) > 0]
    
    if len(data_to_plot) > 0:
        bp = ax1.boxplot(data_to_plot, labels=categories[:len(data_to_plot)], patch_artist=True)
        colors = ['#ff6b6b', '#4ecdc4']
        for patch, color in zip(bp['boxes'], colors):
            patch.set_facecolor(color)
            patch.set_alpha(0.7)
        ax1.set_ylabel('Transitive Proof Effort (log scale)', fontsize=12)
        ax1.set_yscale('log')
        ax1.set_title('Proof Effort Distribution by Category', fontsize=14)
    
    # Plot 5b: Histogram of proof depth
    ax2 = axes[1]
    df_depth = df[df['proof_depth'].notna() & df['non_trivial_proof']]
    if len(df_depth) > 0:
        ax2.hist(df_depth['proof_depth'], bins=range(0, int(df_depth['proof_depth'].max()) + 2),
                 color='steelblue', edgecolor='white', alpha=0.7)
        ax2.set_xlabel('Proof Depth', fontsize=12)
        ax2.set_ylabel('Number of Functions', fontsize=12)
        ax2.set_title('Distribution of Proof Depths', fontsize=14)
        ax2.axvline(df_depth['proof_depth'].mean(), color='red', linestyle='--', 
                    label=f'Mean: {df_depth["proof_depth"].mean():.1f}')
        ax2.legend()
    
    plt.tight_layout()
    plt.savefig(output_dir / 'plot5_proof_distribution.png', dpi=150, bbox_inches='tight')
    plt.close()
    print("✓ Saved plot5_proof_distribution.png")


def plot_combined_complexity(df: pd.DataFrame, output_dir: Path):
    """Plot 6: Combined view of code + spec vs proof."""
    fig, ax = plt.subplots(figsize=(12, 8))
    
    df_full = df[
        df['non_trivial_proof'] &
        df['halstead_effort'].notna() &
        df['ensures_halstead_effort'].notna() &
        df['transitive_proof_effort'].notna()
    ].copy()
    
    if len(df_full) == 0:
        print("Warning: No data for combined complexity plot")
        return
    
    # Combined complexity = code effort + spec effort
    df_full['combined_complexity'] = df_full['halstead_effort'] + df_full['ensures_halstead_effort']
    
    scatter = ax.scatter(
        df_full['combined_complexity'],
        df_full['transitive_proof_effort'],
        c=df_full['transitive_lemmas_count'],
        cmap='viridis',
        s=80,
        alpha=0.7
    )
    
    ax.set_xlabel('Combined Complexity (Code + Spec Effort)', fontsize=12)
    ax.set_ylabel('Transitive Proof Effort', fontsize=12)
    ax.set_title('Combined Code+Spec Complexity vs Proof Effort', fontsize=14)
    ax.set_xscale('log')
    ax.set_yscale('log')
    
    plt.colorbar(scatter, label='Transitive Lemmas Count')
    
    # Correlation
    corr = df_full[['combined_complexity', 'transitive_proof_effort']].corr().iloc[0, 1]
    ax.text(0.05, 0.95, f'r = {corr:.3f}', transform=ax.transAxes, fontsize=11,
            verticalalignment='top', bbox=dict(boxstyle='round', facecolor='wheat', alpha=0.5))
    
    # Add annotation for outliers (top 3)
    top3 = df_full.nlargest(3, 'transitive_proof_effort')
    for _, row in top3.iterrows():
        ax.annotate(
            row['function'].split('::')[-1],
            (row['combined_complexity'], row['transitive_proof_effort']),
            textcoords="offset points", xytext=(5, 5), fontsize=8
        )
    
    plt.tight_layout()
    plt.savefig(output_dir / 'plot6_combined_complexity.png', dpi=150, bbox_inches='tight')
    plt.close()
    print("✓ Saved plot6_combined_complexity.png")


def generate_summary_stats(df: pd.DataFrame, output_dir: Path):
    """Generate summary statistics."""
    df_proofs = df[df['non_trivial_proof']]
    
    summary = f"""
# Metrics Correlation Analysis Summary

## Dataset Overview
- Total functions: {len(df)}
- Functions with proofs: {len(df[df['has_proof'] == 'yes'])}
- Non-trivial proofs: {len(df_proofs)}
- Functions with RCA metrics: {df['halstead_length'].notna().sum()}

## Key Correlations (Non-trivial proofs only)

"""
    
    # Compute correlations
    correlations = []
    pairs = [
        ('halstead_length', 'transitive_proof_effort', 'Code Length → Proof Effort'),
        ('cyclomatic', 'transitive_lemmas_count', 'Cyclomatic → Lemmas Count'),
        ('ensures_halstead_effort', 'transitive_proof_effort', 'Spec Effort → Proof Effort'),
        ('ensures_halstead_length', 'direct_lemmas_count', 'Spec Length → Direct Lemmas'),
    ]
    
    for col1, col2, desc in pairs:
        if col1 in df_proofs.columns and col2 in df_proofs.columns:
            valid = df_proofs[[col1, col2]].dropna()
            if len(valid) > 5:
                corr = valid.corr().iloc[0, 1]
                correlations.append((desc, corr, len(valid)))
                summary += f"| {desc} | r = {corr:.3f} | n = {len(valid)} |\n"
    
    summary += """
## Insights

"""
    
    # Add insights based on correlations
    for desc, corr, n in correlations:
        if abs(corr) > 0.7:
            summary += f"- **Strong correlation** ({corr:.2f}): {desc}\n"
        elif abs(corr) > 0.4:
            summary += f"- **Moderate correlation** ({corr:.2f}): {desc}\n"
        elif abs(corr) < 0.2:
            summary += f"- **Weak/no correlation** ({corr:.2f}): {desc} - interesting!\n"
    
    # Top proof overhead functions
    df_overhead = df[df['proof_overhead_ratio'].notna() & df['non_trivial_proof']]
    if len(df_overhead) > 0:
        top5 = df_overhead.nlargest(5, 'proof_overhead_ratio')
        summary += """
## Top 5 Functions by Proof Overhead

| Function | Proof Overhead Ratio |
|----------|---------------------|
"""
        for _, row in top5.iterrows():
            summary += f"| {row['function']} | {row['proof_overhead_ratio']:.1f}x |\n"
    
    with open(output_dir / 'ANALYSIS_SUMMARY.md', 'w') as f:
        f.write(summary)
    
    print("✓ Saved ANALYSIS_SUMMARY.md")


def main():
    if len(sys.argv) < 2:
        print("Usage: python visualize_metrics.py <csv_file> [output_dir]")
        print("\nExample:")
        print("  python scripts/visualize_metrics.py data/csv/pipeline_FINAL.csv")
        sys.exit(1)
    
    csv_path = sys.argv[1]
    output_dir = Path(sys.argv[2]) if len(sys.argv) > 2 else Path('data/plots')
    
    # Create output directory
    output_dir.mkdir(parents=True, exist_ok=True)
    
    print(f"Loading data from {csv_path}...")
    df = load_and_clean_data(csv_path)
    print(f"  Loaded {len(df)} functions")
    print(f"  Non-trivial proofs: {df['non_trivial_proof'].sum()}")
    print()
    
    print("Generating plots...")
    print("-" * 50)
    
    plot_code_vs_proof(df, output_dir)
    plot_spec_vs_proof(df, output_dir)
    plot_correlation_heatmap(df, output_dir)
    plot_proof_overhead(df, output_dir)
    plot_proof_distribution(df, output_dir)
    plot_combined_complexity(df, output_dir)
    generate_summary_stats(df, output_dir)
    
    print("-" * 50)
    print(f"\n✅ All plots saved to {output_dir}/")
    print("\nGenerated files:")
    for f in sorted(output_dir.glob('*')):
        print(f"  • {f.name}")


if __name__ == '__main__':
    main()

