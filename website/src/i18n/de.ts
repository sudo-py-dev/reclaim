import type { Dict } from './en';

const de: Dict = {
  meta: {
    title: 'Reclaim | Gelöschte Dateien kostenlos wiederherstellen unter Windows, macOS & Linux',
    description:
      'Gelöschte Fotos, Dokumente und andere Dateien von USB-Sticks, SD-Karten und Festplatten wiederherstellen. Reclaim ist kostenlos, Open Source und liest nur. NTFS, FAT32, exFAT, ext2/3/4 und HFS+.',
    downloadTitle: 'Reclaim herunterladen | Kostenlose Datenrettung für Windows, macOS & Linux',
    downloadDescription:
      'Den neuesten Reclaim-Installer für Windows, macOS (Apple Silicon und Intel) oder Debian und Ubuntu herunterladen. Kostenlose Open-Source-Wiederherstellung gelöschter Dateien.',
    notFoundTitle: 'Seite nicht gefunden | Reclaim',
    ogAlt: 'Reclaim, eine Desktop-App zum Wiederherstellen gelöschter Dateien',
  },
  nav: {
    skip: 'Zum Inhalt springen',
    how: 'So funktioniert’s',
    features: 'Funktionen',
    formats: 'Dateitypen',
    faq: 'FAQ',
    download: 'Download',
    github: 'GitHub',
    language: 'Sprache',
    home: 'Reclaim-Startseite',
    menu: 'Menü',
  },
  hero: {
    eyebrow: 'Kostenlos und Open Source · Windows, macOS, Linux',
    title: 'Gelöschte Dateien wiederherstellen, ohne alles schlimmer zu machen.',
    lead:
      'Reclaim durchsucht das Laufwerk schreibgeschützt, zeigt dir, was es findet, und speichert die ausgewählten Dateien an einem sicheren Ort. Fotos, Dokumente, Archive und mehr von USB-Sticks, SD-Karten und Festplatten.',
    cta: 'Reclaim herunterladen',
    ctaFor: 'Für {os} herunterladen',
    secondary: 'Alle Downloads',
    meta: 'Version {version}',
    released: 'Veröffentlicht am {date}',
    license: 'MIT-Lizenz',
    shotDrives: 'Reclaim zeigt eine Liste durchsuchbarer Laufwerke, ein USB-Stick ist ausgewählt',
    shotResults: 'Reclaim zeigt wiederherstellbare Dateien mit Suche und Kategoriefiltern',
  },
  warn: {
    title: 'Dateien verloren? Schreib nichts mehr auf das Laufwerk.',
    body:
      'Eine gelöschte Datei bleibt meist auf dem Laufwerk, bis neue Daten darüber geschrieben werden. Jedes Speichern, jeder Download, jede Installation und jedes Update kann diesen Platz belegen.',
    external: {
      lead: 'Auf Speicherkarte, USB-Stick oder externer Festplatte',
      text: 'Zieh sie ab und mach keine neuen Fotos damit. Bieten Computer oder Kamera an, sie zu formatieren: ablehnen. Reclaim kannst du ganz normal auf deinem Computer installieren.',
    },
    internal: {
      lead: 'Auf dem Hauptlaufwerk deines Computers',
      text: 'Installiere dort nichts. Lade die portable Version auf einem anderen Computer oder direkt auf einen USB-Stick herunter, starte Reclaim vom Stick und speichere gerettete Dateien auf einem anderen Laufwerk.',
    },
  },
  steps: {
    kicker: 'So funktioniert’s',
    title: 'In vier Schritten von „Oh nein“ zu „Geschafft“.',
    items: [
      {
        title: 'Laufwerk wählen',
        body: 'Wähle eine Festplatte, einen USB-Stick, eine SD-Karte oder ein Disk-Image. Die Liste aktualisiert sich, sobald du etwas anschließt.',
      },
      {
        title: 'Durchsuchen',
        body: 'Ergebnisse erscheinen schon während der Suche. Halte sie an, sobald du findest, was du brauchst, und arbeite mit den Treffern weiter.',
      },
      {
        title: 'Prüfen',
        body: 'Suche nach Name oder Typ, filtere nach Kategorie, sieh dir Bildvorschauen und die Details jeder Datei an, bevor du entscheidest.',
      },
      {
        title: 'Woanders speichern',
        body: 'Stelle die ausgewählten Dateien in einem Ordner auf einem anderen Laufwerk wieder her, damit sie nichts überschreiben, was du noch retten willst.',
      },
    ],
  },
  features: {
    kicker: 'Für sorgfältige Wiederherstellung gebaut',
    title: 'Einfach an der Oberfläche. Vorsichtig im Kern.',
    items: [
      {
        title: 'Nur lesend',
        body: 'Reclaim schreibt nie auf das durchsuchte Laufwerk. Es liest die Belegungstabelle des Dateisystems und durchsucht nur freien Speicher. Vorhandene Dateien bleiben unberührt.',
      },
      {
        title: 'Erst prüfen, dann speichern',
        body: 'Dateitypsymbole, Suche, Kategoriefilter, Bildvorschau und ein Datei-Inspektor helfen, echte Funde von Datenmüll zu unterscheiden.',
      },
      {
        title: 'Für gängige Laufwerke',
        body: 'NTFS, FAT12/16/32, exFAT, ext2/3/4 und HFS+: die Formate von Windows- und Linux-Laufwerken, externen Mac-Laufwerken, USB-Sticks, SD-Karten und Kamera-Speicherkarten.',
      },
      {
        title: 'In deiner Sprache',
        body: 'Englisch, Hebräisch, Arabisch, Spanisch, Französisch, Deutsch und Russisch, mit Rechts-nach-links-Layout sowie hellem und dunklem Design.',
      },
      {
        title: 'Sichere Updates',
        body: 'Reclaim sucht nach neuen Versionen und prüft Signatur und Prüfsumme jedes Updates vor der Installation. Scandaten verlassen nie deinen Computer.',
      },
      {
        title: 'Kostenlos und Open Source',
        body: 'Keine Testversion, kein Konto, kein Upselling. Der vollständige Quellcode liegt unter MIT-Lizenz auf GitHub.',
      },
    ],
  },
  formats: {
    kicker: 'Was es findet',
    title: 'Die Dateien, die am häufigsten verloren gehen.',
    lead: 'Reclaim erkennt Dateien an ihrem Inhalt und findet sie daher auch, wenn der Name verloren ist.',
    rows: [
      { name: 'Fotos und Bilder', list: 'JPEG, PNG, GIF, BMP, WebP, QOI, ICO' },
      { name: 'Dokumente und E-Books', list: 'PDF, Word, Excel, PowerPoint, OpenDocument, EPUB' },
      { name: 'Archive', list: 'ZIP, 7z, RAR5, CAB, TAR, GZIP, XAR, JAR, APK' },
      { name: 'Programme und Installer', list: 'EXE, DLL, ELF, MSI, DEB, RPM, DEX, PKG' },
      { name: 'Audio und Video', list: 'WAV, AIFF, AU, MIDI, AVI, Ogg Vorbis, Opus, Theora, Flash (SWF)' },
      { name: 'Datenbanken', list: 'SQLite' },
    ],
    more: 'Vollständige Formatliste',
  },
  limits: {
    kicker: 'Ehrliche Grenzen',
    title: 'Was Reclaim (noch) nicht kann.',
    lead: 'Reclaim ist eine frühe Vorschau, und eine Wiederherstellung ist nie garantiert. Kenne die Grenzen, bevor du anfängst.',
    items: [
      'Überschriebene oder per SSD-TRIM gelöschte Daten lassen sich nicht zurückholen.',
      'Ursprüngliche Dateinamen und Ordner werden nicht wiederhergestellt; Dateien erhalten generierte Namen.',
      'Die meisten Dateien sind auf 64 MiB begrenzt, fragmentierte Dateien können unvollständig sein.',
      'Interne Mac-Laufwerke (APFS), Linux-Partitionen mit Btrfs oder XFS und komplette Disk-Images mit Partitionstabelle können noch nicht durchsucht werden.',
      'Ergebnisse gehen beim Beenden verloren, also speichere zuerst, was du brauchst.',
    ],
  },
  faq: {
    title: 'Häufige Fragen',
    items: [
      {
        q: 'Ist Reclaim wirklich kostenlos?',
        a: 'Ja. Reclaim ist kostenlos und Open Source unter der MIT-Lizenz. Es gibt keine Bezahlversion, keine Wiederherstellungslimits und kein Konto.',
      },
      {
        q: 'Kann ich Dateien nach dem Leeren des Papierkorbs wiederherstellen?',
        a: 'Liegen die Dateien noch im Papierkorb, stelle sie dort wieder her. Das geht schneller, und die Namen bleiben erhalten. Nach dem Leeren bleiben die Daten auf Festplatten, Speicherkarten und USB-Sticks meist erhalten, bis etwas sie überschreibt; ein Scan hat also gute Chancen. Auf der internen SSD eines Computers werden sie dagegen oft schon kurz danach gelöscht (siehe Frage zu SSDs).',
      },
      {
        q: 'Kann es Fotos von einer SD-Karte oder einem USB-Stick retten?',
        a: 'In der Regel ja: SD-Karten und die meisten USB-Sticks verwenden FAT32 oder exFAT, die Reclaim unterstützt. Benutze die Karte sofort nicht mehr, mach keine neuen Fotos und lehne jedes Angebot zum Formatieren ab. Schließe sie dann über einen Kartenleser an, wähle sie in Reclaim aus und starte den Scan.',
      },
      {
        q: 'Kann der Scan mein Laufwerk verändern oder beschädigen?',
        a: 'Nein. Reclaim liest nur vom durchsuchten Laufwerk. Speichere gerettete Dateien auf einem anderen Laufwerk, denn auf demselben könnten sie Dateien überschreiben, die du noch retten willst.',
      },
      {
        q: 'Funktioniert es mit SSDs?',
        a: 'Interne SSDs behalten gelöschte Dateien meist nicht lange. Das System teilt der SSD mit, welcher Speicher frei ist (das nennt sich TRIM), und die SSD löscht ihn. Unter Windows und auf dem Mac passiert das meist direkt nach dem Löschen, unter Linux oft erst bei einer wöchentlichen Bereinigung. Danach kann keine Software die Daten zurückholen. USB-Sticks und viele ältere externe Laufwerke erhalten kein TRIM, dort sind die Chancen besser. Moderne externe SSDs erhalten es jedoch oft.',
      },
      {
        q: 'Was, wenn das Laufwerk klickt, sehr langsam ist oder sich ständig trennt?',
        a: 'Benutze es nicht weiter und scanne es nicht. Ein defektes Laufwerk kann mit jedem Lesezugriff schlechter werden, und keine Software repariert Hardware. Wenn die Daten wichtig sind, schalte es ab und wende dich an einen professionellen Datenrettungsdienst.',
      },
      {
        q: 'Warum haben wiederhergestellte Dateien andere Namen?',
        a: 'Reclaim findet Dateien anhand ihres Inhalts im freien Speicher, wo Name und Ordner meist nicht mehr verzeichnet sind. Sie werden mit generierten Namen gespeichert. Öffne sie, um zu prüfen, was sie enthalten.',
      },
    ],
  },
  cta: {
    title: 'Je früher du scannst, desto besser die Chancen.',
    body: 'Reclaim ist kostenlos, und der Installer ist kleiner als 10 MB.',
  },
  download: {
    title: 'Reclaim herunterladen',
    lead: 'Kostenlos für Windows, macOS und Linux. Jeder Download ist signiert und hat eine SHA-256-Prüfsumme.',
    version: 'Neueste Version: {version}',
    notes: 'Versionshinweise',
    recommended: 'Empfohlen für dein System',
    installer: 'Installer',
    portable: 'Portables ZIP',
    platforms: {
      windows: {
        name: 'Windows',
        detail: '64-Bit (x86_64)',
        scans: 'Durchsucht NTFS-, FAT32- und exFAT-Laufwerke: interne Laufwerke, Speicherkarten und USB-Sticks. Externe Laufwerke mit ext2/3/4 oder HFS+ funktionieren ebenfalls.',
        how: 'Starte den Installer. Er installiert Reclaim für deinen Benutzer, mit Startmenü-Verknüpfung und Deinstallationsprogramm. Lässt sich ein Laufwerk nicht öffnen, klicke mit der rechten Maustaste auf Reclaim und wähle „Als Administrator ausführen“. Dateien auf Laufwerk C: verloren? Nutze stattdessen die portable Version von einem USB-Stick.',
      },
      macos: {
        name: 'macOS',
        detail: 'Apple Silicon (ab M1) oder Intel',
        scans: 'Durchsucht Speicherkarten, USB-Sticks und externe Laufwerke mit FAT32, exFAT, NTFS oder Mac OS Extended (HFS+). Das interne Laufwerk deines Macs (APFS) wird noch nicht unterstützt.',
        how: 'Öffne das Paket, um Reclaim in „Programme“ zu installieren. Die App ist noch nicht von Apple notarisiert: Blockiert macOS sie, öffne Systemeinstellungen → Datenschutz & Sicherheit und klicke auf „Dennoch öffnen“. Zum Lesen eines ganzen Laufwerks ist ein Administratorpasswort nötig, und macOS fragt eventuell nach Zugriff auf Wechselmedien.',
      },
      linux: {
        name: 'Linux',
        detail: 'Debian 12, Ubuntu 22.04 oder neuer · x86_64',
        scans: 'Durchsucht ext2-, ext3- und ext4-Partitionen sowie FAT32-, exFAT- und NTFS-Laufwerke wie Speicherkarten und USB-Sticks. Btrfs und XFS werden noch nicht unterstützt.',
        how: 'Installiere das Paket im Download-Ordner mit dem folgenden Befehl. Wenn du ein Laufwerk auswählst, fragt Reclaim über die übliche Systemabfrage nach einem Administratorpasswort. Portable Versionen benötigen glibc 2.35 oder neuer.',
      },
    },
    arm: 'Apple Silicon',
    intel: 'Intel',
    deb: '.deb-Paket',
    exe: 'Windows-Installer',
    copy: 'Kopieren',
    copied: 'Kopiert',
    verifyTitle: 'Download prüfen',
    verifyBody:
      'Zu jeder Datei gibt es auf der Release-Seite eine .sha256-Prüfsumme und eine .minisig-Signatur. Der eingebaute Updater von Reclaim prüft beides automatisch.',
    allFiles: 'Alle Dateien auf GitHub',
    sourceTitle: 'Aus dem Quellcode bauen',
    sourceBody: 'Reclaim ist in Rust geschrieben. Installiere stabiles Rust und die Build-Werkzeuge deines Systems und führe dann aus:',
  },
  footer: {
    tagline: 'Kostenlose Open-Source-Wiederherstellung gelöschter Dateien.',
    product: 'Produkt',
    project: 'Projekt',
    source: 'Quellcode',
    releases: 'Versionen',
    issues: 'Problem melden',
    docs: 'Entwicklerhandbuch',
    license: 'Veröffentlicht unter der MIT-Lizenz.',
  },
  notFound: {
    title: 'Diese Seite gibt es nicht.',
    body: 'Anders als deine Dateien lässt sie sich nicht wiederherstellen. Versuch es mit der Startseite.',
    back: 'Zur Startseite',
  },
  ui: {
    windowTitle: 'Reclaim - Dateiwiederherstellung',
    zoom: 'Screenshot vergrößern',
    close: 'Schließen',
    tour: 'Produkttour',
  },
  os: { windows: 'Windows', macos: 'macOS', linux: 'Linux' },
};

export default de;
