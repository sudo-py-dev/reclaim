# Reclaim website

The public site for Reclaim: a static [Astro](https://astro.build) site in seven
languages, deployed on Vercel.

```sh
cd website
npm install
npm run dev      # http://localhost:4321
npm run build    # static output in dist/
npm run check    # type-check .astro and .ts files
```

## Deploying on Vercel

1. Import the repository in Vercel and set **Root Directory** to `website`.
   The framework (Astro), build command and output directory come from `vercel.json`.
2. Optional environment variables:
   - `SITE_URL`: the public origin, for example `https://reclaim.example.com`.
     It's used for canonical URLs, hreflang, Open Graph and the sitemap. Without it,
     the project's production `*.vercel.app` domain is used.
   - `GITHUB_TOKEN`: a read-only token. It avoids GitHub API rate limits when
     fetching the latest release during builds.
3. Submit `https://<your domain>/sitemap-index.xml` in Google Search Console.

## How download links stay current

- Windows and macOS buttons use GitHub's `/releases/latest/download/<asset>` URLs,
  so they always serve the newest release.
- The Debian package name includes the version. The build reads the latest
  release from the GitHub API, and the page re-checks the API in the browser once per
  session. If there is a newer release, the `.deb` link, the install command and the
  version labels update in place.
- To rebuild the site on every release, create a Deploy Hook in Vercel
  (Settings → Git → Deploy Hooks) and save its URL as the `VERCEL_DEPLOY_HOOK`
  secret in GitHub. The release workflow calls it after publishing.
- Short links for sharing: `/dl/windows`, `/dl/macos-arm`, `/dl/macos-intel`,
  `/github`, `/releases` (see `vercel.json`).

Asset names must stay in sync with `.github/workflows/release.yml` and
`src/updater.rs`.

## Translations

Copy lives in `src/i18n/<locale>.ts`. TypeScript requires every locale to match
`en.ts`, so a missing string fails `npm run check`. English is served at `/`; other
languages are served at `/<locale>/`. Hebrew and Arabic render right to left.

To add a language, create its file, register it in `src/i18n/index.ts` and in the
sitemap `i18n.locales` map in `astro.config.mjs`.

## SEO checklist

- One `<h1>` per page, with the main search phrase for each language in the
  title, description and headings.
- `rel=canonical`, `hreflang` for every language plus `x-default`, and a localized
  sitemap with alternates.
- JSON-LD: `SoftwareApplication`, `WebSite`, `FAQPage` (home) and
  `BreadcrumbList` (download).
- Open Graph and Twitter cards (`public/og.png`, 1200×630).
- Static HTML, inlined CSS, AVIF/WebP screenshots with explicit sizes, and only a
  few kilobytes of JavaScript, for good Core Web Vitals.
