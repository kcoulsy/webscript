import {themes as prismThemes} from 'prism-react-renderer';
import type {Config} from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';

const config: Config = {
  title: 'WebScript',
  tagline: 'Typed, HTML-native web apps with no required build step',
  favicon: 'img/favicon.ico',

  future: {
    v4: true,
  },

  url: 'https://webscript.dev',
  baseUrl: '/',

  organizationName: 'webscript',
  projectName: 'webscript',

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
          routeBasePath: 'docs',
        },
        blog: false,
        theme: {
          customCss: './src/css/custom.css',
        },
      } satisfies Preset.Options,
    ],
  ],

  themeConfig: {
    image: 'img/docusaurus-social-card.jpg',
    colorMode: {
      respectPrefersColorScheme: true,
    },
    navbar: {
      title: 'WebScript',
      logo: {
        alt: 'WebScript',
        src: 'img/logo.svg',
      },
      items: [
        {
          type: 'docSidebar',
          sidebarId: 'docsSidebar',
          position: 'left',
          label: 'Docs',
        },
        {
          href: 'https://github.com/webscript/webscript',
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
            {label: 'Overview', to: '/docs/intro'},
            {label: 'Tutorial', to: '/docs/tutorial'},
            {label: 'Language', to: '/docs/language'},
            {label: 'Standard Library', to: '/docs/stdlib'},
          ],
        },
        {
          title: 'More',
          items: [
            {label: 'Guides', to: '/docs/guides'},
            {label: 'GitHub', href: 'https://github.com/webscript/webscript'},
          ],
        },
      ],
      copyright: `Copyright © ${new Date().getFullYear()} WebScript.`,
    },
    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.dracula,
      additionalLanguages: ['rust'],
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
