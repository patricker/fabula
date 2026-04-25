import type {SidebarsConfig} from '@docusaurus/plugin-content-docs';

const sidebars: SidebarsConfig = {
  docsSidebar: [
    'getting-started',
    {
      type: 'category',
      label: 'Learn Sifting',
      items: [
        'learn/what-is-sifting',
        'learn/sifting-by-example',
        'learn/patterns-from-first-principles',
        'learn/thinking-in-time',
        'learn/interactive-tutorial',
        'learn/dsl-quick-reference',
      ],
    },
    {
      type: 'category',
      label: 'Build a Simulation Monitor',
      items: [
        'build/overview',
        'build/simulation-loop',
        'build/define-patterns',
        'build/incremental-matching',
        'build/react-to-events',
        'build/score-and-rank',
        'build/speculate-with-mcts',
      ],
    },
    {
      type: 'category',
      label: 'Use Cases',
      items: [
        'use-cases/narrative-sifting',
        'use-cases/observability',
        'use-cases/process-mining',
        'use-cases/compliance-checking',
        'use-cases/cybersecurity',
        'use-cases/simulation-monitoring',
      ],
    },
    {
      type: 'category',
      label: 'Playground',
      items: [
        'playground/pattern-playground',
        'playground/step-through',
        'playground/allen-visualizer',
        'playground/scoring-explorer',
      ],
    },
    {
      type: 'category',
      label: 'Concepts',
      items: [
        'concepts/overview',
        'concepts/design-decisions',
        'concepts/how-the-engine-works',
        'concepts/temporal-model',
        'concepts/composition',
        'concepts/scoring-and-surprise',
        'concepts/narrative-quality',
      ],
    },
    {
      type: 'category',
      label: 'Guides',
      items: [
        'guides/pattern-cookbook',
        'guides/incremental-integration',
        'guides/scoring-matches',
        'guides/composing-patterns',
        'guides/computed-bindings',
        'guides/forking-for-mcts',
        'guides/dsl-in-rust',
        'guides/debugging-patterns',
        'guides/custom-adapter',
        'guides/golden-tests',
        'guides/language-integration',
        'guides/performance',
        'guides/troubleshooting',
        {
          type: 'category',
          label: 'Causality',
          items: [
            'guides/tracing-causal-chains',
            'guides/detecting-surprising-events',
          ],
        },
      ],
    },
    {
      type: 'category',
      label: 'Reference',
      items: [
        'reference/interval',
        'reference/datasource',
        'reference/patterns',
        'reference/engine',
        'reference/causality',
        'reference/scoring',
        'reference/narratives',
        'reference/dsl',
        {
          type: 'category',
          label: 'Adapters',
          items: [
            'reference/adapters/memory',
            'reference/adapters/petgraph',
            'reference/adapters/grafeo',
          ],
        },
      ],
    },
    'glossary',
    'learning-paths',
    'research',
  ],
};

export default sidebars;
