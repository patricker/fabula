import type {SidebarsConfig} from '@docusaurus/plugin-content-docs';

const sidebars: SidebarsConfig = {
  docsSidebar: [
    'getting-started',
    {
      type: 'category',
      label: 'Playground',
      items: [
        'playground/pattern-playground',
        'playground/step-through',
        'playground/allen-visualizer',
      ],
    },
    {
      type: 'category',
      label: 'Concepts',
      items: [
        'concepts/overview',
        'concepts/how-the-engine-works',
        'concepts/temporal-model',
        'concepts/design-decisions',
      ],
    },
    {
      type: 'category',
      label: 'Guides',
      items: [
        'guides/pattern-cookbook',
        'guides/incremental-integration',
        'guides/debugging-patterns',
        'guides/custom-adapter',
        'guides/golden-tests',
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
    'research',
  ],
};

export default sidebars;
