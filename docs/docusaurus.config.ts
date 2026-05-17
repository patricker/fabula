import {themes as prismThemes} from 'prism-react-renderer';
import type {Config} from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';
import remarkCodeRegion from './plugins/remark-code-region';

const config: Config = {
  title: 'Fabula',
  tagline: 'Incremental pattern matching over temporal graphs',
  favicon: 'img/favicon.ico',

  future: {
    v4: true,
  },

  // GitHub Pages (custom domain)
  url: 'https://sifting.dev',
  baseUrl: '/',
  organizationName: 'patricker',
  projectName: 'fabula',
  trailingSlash: false,

  onBrokenLinks: 'throw',

  headTags: [
    {tagName: 'link', attributes: {rel: 'apple-touch-icon', sizes: '180x180', href: '/img/apple-touch-icon.png'}},
    {tagName: 'link', attributes: {rel: 'icon', type: 'image/png', sizes: '32x32', href: '/img/favicon-32x32.png'}},
    {tagName: 'link', attributes: {rel: 'icon', type: 'image/png', sizes: '16x16', href: '/img/favicon-16x16.png'}},
  ],

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
          editUrl: 'https://github.com/patricker/fabula/tree/main/docs/',
          remarkPlugins: [remarkCodeRegion],
        },
        blog: false, // No blog for a library
        gtag: {
          trackingID: 'G-XRTR9HXEK3',
          anonymizeIP: true,
        },
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
      title: '',
      logo: {
        alt: 'Fabula',
        src: 'img/wordmark.svg',
        srcDark: 'img/wordmark-dark.svg',
      },
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
          href: 'https://github.com/patricker/fabula',
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
            { label: 'GitHub', href: 'https://github.com/patricker/fabula' },
            { label: 'crates.io', href: 'https://crates.io/crates/fabula' },
            { label: 'docs.rs', href: 'https://docs.rs/fabula' },
          ],
        },
      ],
      copyright: `Copyright © ${new Date().getFullYear()} Fabula contributors. MIT.`,
    },
    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.dracula,
      additionalLanguages: ['rust', 'toml', 'clojure'],
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
