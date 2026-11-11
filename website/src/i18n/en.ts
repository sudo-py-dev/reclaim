const en = {
  meta: {
    title: 'Reclaim | Free Deleted File Recovery for Windows, macOS & Linux',
    description:
      'Recover deleted photos, documents and other files from USB drives, SD cards and hard drives. Reclaim is free, open-source and scans read-only. NTFS, FAT32 and exFAT.',
    downloadTitle: 'Download Reclaim | Free File Recovery for Windows, macOS & Linux',
    downloadDescription:
      'Download the latest Reclaim installer for Windows, macOS (Apple Silicon and Intel) or Debian and Ubuntu. Free, open-source deleted file recovery, signed and checksummed.',
    notFoundTitle: 'Page not found | Reclaim',
    ogAlt: 'Reclaim, a desktop app for recovering deleted files',
  },
  nav: {
    skip: 'Skip to content',
    how: 'How it works',
    features: 'Features',
    formats: 'File types',
    faq: 'FAQ',
    download: 'Download',
    github: 'GitHub',
    language: 'Language',
    home: 'Reclaim home',
    menu: 'Menu',
  },
  hero: {
    eyebrow: 'Free and open source · Windows, macOS, Linux',
    title: 'Recover deleted files without making things worse.',
    lead:
      'Reclaim scans a drive read-only, shows you what it can find, and saves the files you choose to somewhere safe. Photos, documents, archives and more from USB drives, SD cards and hard drives.',
    cta: 'Download Reclaim',
    ctaFor: 'Download for {os}',
    secondary: 'All downloads',
    meta: 'Version {version}',
    released: 'Released {date}',
    license: 'MIT licence',
    shotDrives: 'Reclaim showing a list of drives to scan, with a USB drive selected',
    shotResults: 'Reclaim showing recovered file candidates with search and category filters',
  },
  warn: {
    title: 'Lost files? Stop writing to that drive.',
    body:
      'A deleted file usually stays on the drive until new data lands on top of it. Every save, download, install or update can take that space.',
    external: {
      lead: 'On a memory card, USB stick or external drive',
      text: 'Unplug it and don’t take new photos with it. If your computer or camera offers to format it, say no. You can install Reclaim on your computer as usual.',
    },
    internal: {
      lead: 'On your computer’s main drive',
      text: 'Don’t install anything there. Download the portable version on another computer, or save it straight to a USB stick, then run Reclaim from the stick and save recovered files to another drive.',
    },
  },
  steps: {
    kicker: 'How it works',
    title: 'Four steps from “oh no” to “got it”.',
    items: [
      {
        title: 'Choose a drive',
        body: 'Pick a connected drive, a USB stick, an SD card or a disk image. The list refreshes as you plug things in.',
      },
      {
        title: 'Scan',
        body: 'Results appear while the scan runs. Stop early whenever you see what you need and keep working with what was found.',
      },
      {
        title: 'Review',
        body: 'Search by name or type, filter by category, preview images and check each file’s details before you decide.',
      },
      {
        title: 'Save elsewhere',
        body: 'Recover the selected files to a folder on a different drive, so they can’t overwrite files you haven’t recovered yet.',
      },
    ],
  },
  features: {
    kicker: 'Built for careful recovery',
    title: 'Simple on the surface. Cautious underneath.',
    items: [
      {
        title: 'Read-only by design',
        body: 'Reclaim never writes to the drive it scans. It reads the file system’s allocation map and only searches free space, so existing files are left alone.',
      },
      {
        title: 'See before you save',
        body: 'File-type icons, search, category filters, image previews and a file inspector help you sort real finds from noise.',
      },
      {
        title: 'Works with common drives',
        body: 'NTFS, FAT12/16/32 and exFAT: the formats used by Windows drives, USB sticks, SD cards and camera memory cards.',
      },
      {
        title: 'In your language',
        body: 'English, Hebrew, Arabic, Spanish, French, German and Russian, with right-to-left layouts and light and dark themes.',
      },
      {
        title: 'Safe updates',
        body: 'Reclaim checks for new versions and verifies every update’s signature and checksum before installing. No scan data ever leaves your computer.',
      },
      {
        title: 'Free and open source',
        body: 'No trial limits, no account, no upsell. The full source code is on GitHub under the MIT licence.',
      },
    ],
  },
  formats: {
    kicker: 'What it finds',
    title: 'The files people lose most.',
    lead: 'Reclaim recognises files by their content, so they can be found even when their name is gone.',
    rows: [
      { name: 'Photos and images', list: 'JPEG, PNG, GIF, BMP, WebP, QOI, ICO' },
      { name: 'Documents and ebooks', list: 'PDF, Word, Excel, PowerPoint, OpenDocument, EPUB' },
      { name: 'Archives', list: 'ZIP, 7z, RAR5, CAB, TAR, GZIP, XAR, JAR, APK' },
      { name: 'Programs and installers', list: 'EXE, DLL, ELF, MSI, DEB, RPM, DEX, PKG' },
      { name: 'Audio and video', list: 'WAV, AIFF, AU, AVI, Ogg Vorbis, Opus, Theora' },
      { name: 'Databases', list: 'SQLite' },
    ],
    more: 'Full format coverage',
  },
  limits: {
    kicker: 'Honest limits',
    title: 'What Reclaim can’t do (yet).',
    lead: 'Reclaim is an early preview and recovery is never guaranteed. You should know the limits before you start.',
    items: [
      'Overwritten data, or data erased by an SSD’s TRIM, cannot be brought back.',
      'Original file names and folders are not restored; files get generated names.',
      'Most files are limited to 64 MiB, and fragmented files may come back incomplete.',
      'Mac built-in drives (APFS), Linux system partitions (ext4, Btrfs) and whole-disk images with partition tables can’t be scanned yet.',
      'Scan results are not kept after you quit, so save what you need first.',
    ],
  },
  faq: {
    title: 'Questions',
    items: [
      {
        q: 'Is Reclaim really free?',
        a: 'Yes. Reclaim is free and open source under the MIT licence. There are no paid tiers, recovery limits or accounts.',
      },
      {
        q: 'Can I recover files after emptying the Recycle Bin or Trash?',
        a: 'If the files are still in the Recycle Bin or Trash, restore them from there: it’s faster and keeps their names. After emptying it, files on hard drives, memory cards and USB sticks usually stay on the disk until something overwrites them, so a scan has a good chance. On a computer’s internal SSD they are often erased soon after (see the SSD question below).',
      },
      {
        q: 'Can it recover photos from an SD card or USB stick?',
        a: 'Usually, yes: SD cards and most USB sticks use FAT32 or exFAT, which Reclaim supports. Stop using the card right away, don’t take new photos with it, and decline any offer to format it. Then connect it with a card reader, choose it in Reclaim and start a scan.',
      },
      {
        q: 'Will scanning change or damage my drive?',
        a: 'No. Reclaim only reads the drive it scans. Save recovered files to a different drive: saving them to the same one could overwrite files you haven’t recovered yet.',
      },
      {
        q: 'Does it work on SSDs?',
        a: 'Internal SSDs usually don’t keep deleted files for long. The system tells the SSD which space is free (this is called TRIM) and the SSD erases it. On Windows and Mac this usually happens right after deletion; on Linux it often waits for a weekly cleanup. Once trimmed, no software can bring the data back. USB sticks and many older external drives don’t receive TRIM, so chances there are better, but modern external SSDs often do.',
      },
      {
        q: 'What if the drive is clicking, very slow or keeps disconnecting?',
        a: 'Stop using it and don’t scan it. A failing drive can get worse with every read, and recovery software can’t fix hardware. If the data matters, turn the drive off and contact a professional data recovery service.',
      },
      {
        q: 'Why do recovered files have different names?',
        a: 'Reclaim finds files by their contents in free space, where the original name and folder are usually no longer recorded. Files are saved with generated names, so open them to check what they are.',
      },
    ],
  },
  cta: {
    title: 'The sooner you scan, the better your chances.',
    body: 'Reclaim is free, and the installer is under 10 MB.',
  },
  download: {
    title: 'Download Reclaim',
    lead: 'Free for Windows, macOS and Linux. Every download is signed and comes with a SHA-256 checksum.',
    version: 'Latest version: {version}',
    notes: 'Release notes',
    recommended: 'Recommended for your system',
    installer: 'Installer',
    portable: 'Portable ZIP',
    platforms: {
      windows: {
        name: 'Windows',
        detail: '64-bit (x86_64)',
        scans: 'Scans NTFS, FAT32 and exFAT drives: internal drives, memory cards and USB sticks.',
        how: 'Run the installer. It installs for your user and adds a Start menu shortcut and an uninstaller. If a drive won’t open, right-click Reclaim and choose Run as administrator. Lost files on drive C:? Use the portable version from a USB stick instead.',
      },
      macos: {
        name: 'macOS',
        detail: 'Apple Silicon (M1 and newer) or Intel',
        scans: 'Scans memory cards, USB sticks and external drives formatted as FAT32, exFAT or NTFS. Your Mac’s built-in drive (APFS) can’t be scanned yet.',
        how: 'Open the package to install Reclaim in Applications. It isn’t notarized by Apple yet, so if macOS blocks it, go to System Settings → Privacy & Security and click Open Anyway. Reading a whole drive needs an administrator password, and macOS may also ask you to allow access to removable volumes.',
      },
      linux: {
        name: 'Linux',
        detail: 'Debian 12, Ubuntu 22.04 or newer · x86_64',
        scans: 'Scans FAT32, exFAT and NTFS drives, such as memory cards and USB sticks. Linux system partitions (ext4, Btrfs) can’t be scanned yet.',
        how: 'In the folder you downloaded it to, install the package with the command below. When you choose a drive, Reclaim asks for an administrator password through the usual system prompt. Portable builds need glibc 2.35 or newer.',
      },
    },
    arm: 'Apple Silicon',
    intel: 'Intel',
    deb: '.deb package',
    exe: 'Windows installer',
    copy: 'Copy',
    copied: 'Copied',
    verifyTitle: 'Verify your download',
    verifyBody:
      'Each file has a matching .sha256 checksum and a .minisig signature on the release page. Reclaim’s built-in updater checks both automatically.',
    allFiles: 'All files on GitHub',
    sourceTitle: 'Build from source',
    sourceBody: 'Reclaim is written in Rust. Install stable Rust and your platform’s build tools, then run:',
  },
  footer: {
    tagline: 'Free, open-source deleted file recovery.',
    product: 'Product',
    project: 'Project',
    source: 'Source code',
    releases: 'Releases',
    issues: 'Report a problem',
    docs: 'Developer guide',
    license: 'Released under the MIT licence.',
  },
  notFound: {
    title: 'This page doesn’t exist.',
    body: 'Unlike your files, this one can’t be recovered. Try the home page instead.',
    back: 'Go to the home page',
  },
  ui: {
    windowTitle: 'Reclaim - File Recovery',
    zoom: 'Enlarge screenshot',
    close: 'Close',
    tour: 'Product tour',
  },
  os: { windows: 'Windows', macos: 'macOS', linux: 'Linux' },
};

export default en;
export type Dict = typeof en;
