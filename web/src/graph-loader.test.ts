import { describe, it, expect } from 'vitest';
import { convertAtomDictToD3Graph, pickSourceConfig } from './graph-loader';
import { ProbeAtom, SourceConfig } from './types';

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

describe('pickSourceConfig', () => {
  const configs: SourceConfig[] = [
    {
      github_url: 'https://github.com/org/importer',
      ref: 'aaa', path_prefix: '', language: 'lean', package: 'Importer',
    },
    {
      github_url: 'https://github.com/org/imported',
      ref: 'bbb', path_prefix: '', language: 'lean', package: 'Imported',
    },
    {
      github_url: 'https://github.com/org/rust-crate',
      ref: 'ccc', path_prefix: 'rust-crate', language: 'rust', package: 'rust-crate',
    },
  ];

  it('matches the package of the node path root, not the first same-language input', () => {
    expect(pickSourceConfig(configs, 'lean', 'Imported/Sub/File.lean')?.github_url)
      .toBe('https://github.com/org/imported');
  });

  it('strips .lean from a root-level file when matching', () => {
    expect(pickSourceConfig(configs, 'lean', 'Importer.lean')?.github_url)
      .toBe('https://github.com/org/importer');
  });

  it('falls back to the first language match when no package matches', () => {
    expect(pickSourceConfig(configs, 'lean', 'Other/File.lean')?.github_url)
      .toBe('https://github.com/org/importer');
  });

  it('only considers configs of the node language', () => {
    expect(pickSourceConfig(configs, 'rust', 'src/lib.rs')?.github_url)
      .toBe('https://github.com/org/rust-crate');
    expect(pickSourceConfig(configs, undefined, 'Imported/File.lean')).toBeUndefined();
  });
});
