// @ts-check
import { defineConfig } from 'astro/config';
import sitemap from '@astrojs/sitemap';

/**
 * The public origin used for canonical URLs, hreflang, Open Graph and the sitemap.
 * Set SITE_URL in Vercel once a custom domain is attached; until then the
 * project's production *.vercel.app domain (provided by Vercel at build time) is used.
 */
function siteUrl() {
  if (process.env.SITE_URL) return process.env.SITE_URL.replace(/\/+$/, '');
  if (process.env.VERCEL_PROJECT_PRODUCTION_URL) {
    return `https://${process.env.VERCEL_PROJECT_PRODUCTION_URL}`;
  }
  return 'http://localhost:4321';
}

export default defineConfig({
  site: siteUrl(),
  trailingSlash: 'always',
  build: { format: 'directory', inlineStylesheets: 'always' },
  compressHTML: true,
  prefetch: { prefetchAll: false, defaultStrategy: 'hover' },
  integrations: [
    sitemap({
      filter: (page) => !page.includes('/404'),
      i18n: {
        defaultLocale: 'en',
        locales: {
          en: 'en',
          he: 'he',
          ar: 'ar',
          es: 'es',
          fr: 'fr',
          de: 'de',
          ru: 'ru',
        },
      },
    }),
  ],
});
