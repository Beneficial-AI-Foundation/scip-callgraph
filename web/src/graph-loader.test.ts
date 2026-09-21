import { describe, it, expect } from 'vitest';
import { convertAtomDictToD3Graph } from './graph-loader';
import { ProbeAtom } from './types';

const atom = (over: Partial<ProbeAtom>): ProbeAtom => ({
  "display-name": 'x',
  dependencies: [],
  "code-text": null,
  "code-path": 'X.lean',
  "code-module": 'X',
  kind: 'theorem',
  language: 'lean',
  ...over,
});

describe('convertAtomDictToD3Graph external dependencies', () => {
  const atoms: Record<string, ProbeAtom> = {
    'probe:caller': atom({
      "display-name": 'caller',
      dependencies: ['probe:helper'],
      "term-dependencies-external": ['probe:other.theorem', 'probe:Unresolved.name'],
      "type-dependencies-external": ['probe:other.Type'],
    }),
    'probe:helper': atom({ "display-name": 'helper' }),
    'probe:other.theorem': atom({ "display-name": 'other.theorem', "code-module": 'Other' }),
    'probe:other.Type': atom({ "display-name": 'other.Type', "code-module": 'Other' }),
  };
  const graph = convertAtomDictToD3Graph(atoms);

  it('adds edges for external deps that resolve in the graph', () => {
    const targets = graph.links
      .filter(l => l.source === 'probe:caller')
      .map(l => l.target)
      .sort();
    expect(targets).toEqual(['probe:helper', 'probe:other.Type', 'probe:other.theorem']);
  });

  it('drops external deps that do not resolve', () => {
    expect(graph.links.some(l => l.target === 'probe:Unresolved.name')).toBe(false);
  });

  it('includes resolved externals in node dependencies and dependents', () => {
    const caller = graph.nodes.find(n => n.id === 'probe:caller')!;
    expect([...caller.dependencies].sort())
      .toEqual(['probe:helper', 'probe:other.Type', 'probe:other.theorem']);
    const target = graph.nodes.find(n => n.id === 'probe:other.theorem')!;
    expect(target.dependents).toEqual(['probe:caller']);
  });

  it('does not duplicate an edge already present in dependencies', () => {
    const dup: Record<string, ProbeAtom> = {
      'probe:a': atom({
        dependencies: ['probe:b'],
        "term-dependencies-external": ['probe:b'],
      }),
      'probe:b': atom({}),
    };
    const g = convertAtomDictToD3Graph(dup);
    expect(g.links.filter(l => l.source === 'probe:a' && l.target === 'probe:b')).toHaveLength(1);
  });
});
