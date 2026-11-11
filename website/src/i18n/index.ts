import en, { type Dict } from './en';
import he from './he';
import ar from './ar';
import es from './es';
import fr from './fr';
import de from './de';
import ru from './ru';

/** Same set and order as the desktop app's language picker. */
export const LOCALES = ['en', 'he', 'ar', 'es', 'fr', 'de', 'ru'] as const;
export type Locale = (typeof LOCALES)[number];
export const DEFAULT_LOCALE: Locale = 'en';

const dicts: Record<Locale, Dict> = { en, he, ar, es, fr, de, ru };

export const LANGUAGE_NAMES: Record<Locale, string> = {
  en: 'English',
  he: 'עברית',
  ar: 'العربية',
  es: 'Español',
  fr: 'Français',
  de: 'Deutsch',
  ru: 'Русский',
};

/** Open Graph locale codes. */
export const OG_LOCALE: Record<Locale, string> = {
  en: 'en_US',
  he: 'he_IL',
  ar: 'ar_AR',
  es: 'es_ES',
  fr: 'fr_FR',
  de: 'de_DE',
  ru: 'ru_RU',
};

const RTL = new Set<Locale>(['he', 'ar']);

export const isLocale = (value: unknown): value is Locale =>
  typeof value === 'string' && (LOCALES as readonly string[]).includes(value);

export const dirOf = (locale: Locale): 'rtl' | 'ltr' => (RTL.has(locale) ? 'rtl' : 'ltr');

export const useDict = (locale: Locale): Dict => dicts[locale];

/** Replace `{name}` placeholders. */
export function fill(template: string, values: Record<string, string>): string {
  return template.replace(/\{(\w+)\}/g, (match, key: string) => values[key] ?? match);
}

export type PageKey = '' | 'download';

/** Root-relative URL of a page in a locale, always with a trailing slash. */
export function localePath(locale: Locale, page: PageKey = ''): string {
  const prefix = locale === DEFAULT_LOCALE ? '' : `/${locale}`;
  const path = page ? `/${page}` : '';
  return `${prefix}${path}/`;
}

/** `getStaticPaths` entries for a page in every locale; English lives at the root. */
export function localeStaticPaths() {
  return LOCALES.map((locale) => ({
    params: { lang: locale === DEFAULT_LOCALE ? undefined : locale },
    props: { locale },
  }));
}
