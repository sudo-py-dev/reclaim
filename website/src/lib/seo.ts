import type { Dict } from '../i18n/en';
import type { Locale } from '../i18n';
import { REPO_URL, type Release } from './release';

/** schema.org SoftwareApplication: eligible for Google's software app rich result. */
export function softwareApp(t: Dict, locale: Locale, release: Release, pageUrl: string, site: URL) {
  return {
    '@context': 'https://schema.org',
    '@type': 'SoftwareApplication',
    name: 'Reclaim',
    description: t.meta.description,
    url: pageUrl,
    applicationCategory: 'UtilitiesApplication',
    applicationSubCategory: 'Data recovery',
    operatingSystem: 'Windows, macOS, Linux',
    softwareVersion: release.version,
    ...(release.date ? { datePublished: release.date.slice(0, 10) } : {}),
    downloadUrl: new URL(locale === 'en' ? '/download/' : `/${locale}/download/`, site).href,
    installUrl: release.notesUrl,
    license: 'https://opensource.org/license/mit',
    isAccessibleForFree: true,
    inLanguage: ['en', 'he', 'ar', 'es', 'fr', 'de', 'ru'],
    image: new URL('/og.png', site).href,
    screenshot: new URL('/og.png', site).href,
    fileSize: '8MB',
    offers: { '@type': 'Offer', price: '0', priceCurrency: 'USD' },
    sameAs: [REPO_URL],
  };
}

export function website(t: Dict, locale: Locale, site: URL) {
  return {
    '@context': 'https://schema.org',
    '@type': 'WebSite',
    name: 'Reclaim',
    url: new URL('/', site).href,
    inLanguage: locale,
    description: t.footer.tagline,
  };
}

export function faqPage(t: Dict) {
  return {
    '@context': 'https://schema.org',
    '@type': 'FAQPage',
    mainEntity: t.faq.items.map((item) => ({
      '@type': 'Question',
      name: item.q,
      acceptedAnswer: { '@type': 'Answer', text: item.a },
    })),
  };
}

export function breadcrumbs(items: { name: string; url: string }[]) {
  return {
    '@context': 'https://schema.org',
    '@type': 'BreadcrumbList',
    itemListElement: items.map((item, i) => ({
      '@type': 'ListItem',
      position: i + 1,
      name: item.name,
      item: item.url,
    })),
  };
}
