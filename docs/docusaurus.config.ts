import {themes as prismThemes} from 'prism-react-renderer';
import type {Config} from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';

const config: Config = {
  title: 'Fabula',
  tagline: 'Incremental pattern matching over temporal graphs',
  favicon: 'img/favicon.ico',

  future: {
    v4: true,
  },

  // GitHub Pages — will be updated when repo is created
  url: 'https://your-org.github.io',
  baseUrl: '/fabula/',
  organizationName: 'your-org',
  projectName: 'fabula',
  trailingSlash: false,

  onBrokenLinks: 'throw',

  i18n: {
    defaultLocale: 'en',
    locales: ['en'],
  },

  presets: [
    [
      'classic',
      {
        docs: {
          sidebarPath: './sidebars.ts',
          editUrl: 'https://github.com/your-org/fabula/tree/main/docs/',
        },
        blog: false, // No blog for a library
        theme: {
          customCss: './src/css/custom.css',
        },
      } satisfies Preset.Options,
    ],
  ],

  themeConfig: {
    colorMode: {
      respectPrefersColorScheme: true,
    },
    navbar: {
      title: 'Fabula',
      items: [
        {
          type: 'docSidebar',
          sidebarId: 'docsSidebar',
          position: 'left',
          label: 'Docs',
        },
        {
          to: '/docs/playground/pattern-playground',
          label: 'Playground',
          position: 'left',
        },
        {
          href: 'https://docs.rs/fabula',
          label: 'API',
          position: 'left',
        },
        {
          href: 'https://github.com/your-org/fabula',
          label: 'GitHub',
          position: 'right',
        },
      ],
    },
    footer: {
      style: 'dark',
      links: [
        {
          title: 'Docs',
          items: [
            { label: 'Getting Started', to: '/docs/getting-started' },
            { label: 'Concepts', to: '/docs/concepts/overview' },
            { label: 'Reference', to: '/docs/reference/interval' },
          ],
        },
        {
          title: 'More',
          items: [
            { label: 'GitHub', href: 'https://github.com/your-org/fabula' },
            { label: 'crates.io', href: 'https://crates.io/crates/fabula' },
            { label: 'docs.rs', href: 'https://docs.rs/fabula' },
          ],
        },
      ],
      copyright: `Copyright © ${new Date().getFullYear()} Fabula contributors. Apache-2.0.`,
    },
    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.dracula,
      additionalLanguages: ['rust', 'toml', 'clojure'],
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
