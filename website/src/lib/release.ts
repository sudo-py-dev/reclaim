/**
 * Latest-release data, fetched once at build time from the GitHub API.
 *
 * Stable asset names (Windows, macOS) use GitHub's `/releases/latest/download/`
 * URLs, so those links never go stale. The Debian package name contains the
 * version, so it comes from the API; a small client script (see
 * `components/ReleaseRefresh.astro`) re-checks the API in the browser so a
 * cached page still points at the newest build after a release.
 */

export const REPO = 'sudo-py-dev/reclaim';
export const REPO_URL = `https://github.com/${REPO}`;
export const RELEASES_URL = `${REPO_URL}/releases/latest`;
const LATEST = `${REPO_URL}/releases/latest/download`;

export type PlatformId =
  | 'windows'
  | 'windows-zip'
  | 'macos-arm'
  | 'macos-arm-zip'
  | 'macos-intel'
  | 'macos-intel-zip'
  | 'linux-deb'
  | 'linux-zip';

export interface Download {
  id: PlatformId;
  os: 'windows' | 'macos' | 'linux';
  asset: string;
  url: string;
  size: number | null;
}

export interface Release {
  version: string;
  tag: string;
  date: string | null;
  notesUrl: string;
  downloads: Record<PlatformId, Download>;
}

/** Used when the API is unreachable at build time (rate limit, offline build). */
const FALLBACK_VERSION = '0.2.4';

const debName = (version: string) => `reclaim_${version}_amd64.deb`;

function baseDownloads(version: string): Record<PlatformId, Download> {
  const d = (id: PlatformId, os: Download['os'], asset: string, url = `${LATEST}/${asset}`): Download => ({
    id,
    os,
    asset,
    url,
    size: null,
  });
  return {
    windows: d('windows', 'windows', 'reclaim-windows-x86_64-setup.exe'),
    'windows-zip': d('windows-zip', 'windows', 'reclaim-windows-x86_64.exe.zip'),
    'macos-arm': d('macos-arm', 'macos', 'reclaim-macos-aarch64.pkg'),
    'macos-arm-zip': d('macos-arm-zip', 'macos', 'reclaim-macos-aarch64.zip'),
    'macos-intel': d('macos-intel', 'macos', 'reclaim-macos-x86_64.pkg'),
    'macos-intel-zip': d('macos-intel-zip', 'macos', 'reclaim-macos-x86_64.zip'),
    'linux-deb': d(
      'linux-deb',
      'linux',
      debName(version),
      `${REPO_URL}/releases/download/v${version}/${debName(version)}`,
    ),
    'linux-zip': d('linux-zip', 'linux', 'reclaim-linux-x86_64.zip'),
  };
}

interface GhAsset {
  name: string;
  size: number;
  browser_download_url: string;
}

let cached: Promise<Release> | undefined;

export function getRelease(): Promise<Release> {
  cached ??= load();
  return cached;
}

async function load(): Promise<Release> {
  const fallback: Release = {
    version: FALLBACK_VERSION,
    tag: `v${FALLBACK_VERSION}`,
    date: null,
    notesUrl: RELEASES_URL,
    downloads: baseDownloads(FALLBACK_VERSION),
  };

  try {
    const headers: Record<string, string> = {
      Accept: 'application/vnd.github+json',
      'User-Agent': 'reclaim-website-build',
    };
    const token = process.env.GITHUB_TOKEN;
    if (token) headers.Authorization = `Bearer ${token}`;

    const res = await fetch(`https://api.github.com/repos/${REPO}/releases/latest`, {
      headers,
      signal: AbortSignal.timeout(10_000),
    });
    if (!res.ok) throw new Error(`GitHub API ${res.status}`);
    const json = (await res.json()) as {
      tag_name: string;
      published_at: string | null;
      html_url: string;
      assets: GhAsset[];
    };

    const version = json.tag_name.replace(/^v/, '');
    const downloads = baseDownloads(version);
    for (const dl of Object.values(downloads)) {
      const asset = json.assets.find((a) => a.name === dl.asset);
      if (asset) {
        dl.size = asset.size;
        if (dl.id === 'linux-deb') dl.url = asset.browser_download_url;
      }
    }
    return {
      version,
      tag: json.tag_name,
      date: json.published_at,
      notesUrl: json.html_url,
      downloads,
    };
  } catch (error) {
    console.warn(`[release] Using fallback v${FALLBACK_VERSION}: ${(error as Error).message}`);
    return fallback;
  }
}

export function formatSize(bytes: number | null, locale: string): string {
  if (!bytes) return '';
  const mb = bytes / (1024 * 1024);
  return `${new Intl.NumberFormat(locale, { maximumFractionDigits: 1 }).format(mb)} MB`;
}

export function formatDate(iso: string | null, locale: string): string {
  if (!iso) return '';
  return new Intl.DateTimeFormat(locale, { dateStyle: 'long' }).format(new Date(iso));
}
