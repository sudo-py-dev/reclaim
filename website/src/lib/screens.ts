import { getImage } from 'astro:assets';
import type { Locale } from '../i18n';

/**
 * App screenshots rendered by the desktop app's own UI test, one per language,
 * screen and theme (see `cargo test slint_workspace` with RECLAIM_PREVIEW_DIR).
 */
export type Screen = 'sources' | 'results';
export const SCREEN_WIDTH = 1160;
export const SCREEN_HEIGHT = 780;

const files = import.meta.glob<{ default: ImageMetadata }>('../assets/screens/*.png', { eager: true });

function source(locale: Locale, screen: Screen, theme: 'light' | 'dark'): ImageMetadata {
  const file = files[`../assets/screens/${locale}-${screen}-${theme}.png`];
  if (!file) throw new Error(`Missing screenshot ${locale}-${screen}-${theme}`);
  return file.default;
}

export interface ThemeSet {
  avif: string;
  webp: string;
}

export interface ScreenImage {
  light: ThemeSet;
  dark: ThemeSet;
  /** Full-size light/dark WebP for the zoom view and the <img> fallback. */
  fullLight: string;
  fullDark: string;
  width: number;
  height: number;
}

const WIDTHS = [580, 870, SCREEN_WIDTH];
const cache = new Map<string, Promise<ScreenImage>>();

async function themeSet(img: ImageMetadata): Promise<ThemeSet & { full: string }> {
  const [avif, webp, full] = await Promise.all([
    getImage({ src: img, format: 'avif', widths: WIDTHS, quality: 70 }),
    getImage({ src: img, format: 'webp', widths: WIDTHS, quality: 82 }),
    getImage({ src: img, format: 'webp', width: SCREEN_WIDTH, quality: 88 }),
  ]);
  return { avif: avif.srcSet.attribute, webp: webp.srcSet.attribute, full: full.src };
}

export function screenImage(locale: Locale, screen: Screen): Promise<ScreenImage> {
  const key = `${locale}-${screen}`;
  let entry = cache.get(key);
  if (!entry) {
    entry = (async () => {
      const [light, dark] = await Promise.all([
        themeSet(source(locale, screen, 'light')),
        themeSet(source(locale, screen, 'dark')),
      ]);
      return {
        light: { avif: light.avif, webp: light.webp },
        dark: { avif: dark.avif, webp: dark.webp },
        fullLight: light.full,
        fullDark: dark.full,
        width: SCREEN_WIDTH,
        height: SCREEN_HEIGHT,
      };
    })();
    cache.set(key, entry);
  }
  return entry;
}

/** A rectangle on the 1160×780 screenshot, in left-to-right coordinates. */
export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Mirror for right-to-left screenshots, then express as percentages. */
export function rectStyle(rect: Rect, rtl: boolean): string {
  const x = rtl ? SCREEN_WIDTH - rect.x - rect.w : rect.x;
  const pct = (n: number, of: number) => `${((n / of) * 100).toFixed(3)}%`;
  return [
    `left:${pct(x, SCREEN_WIDTH)}`,
    `top:${pct(rect.y, SCREEN_HEIGHT)}`,
    `width:${pct(rect.w, SCREEN_WIDTH)}`,
    `height:${pct(rect.h, SCREEN_HEIGHT)}`,
  ].join(';');
}
