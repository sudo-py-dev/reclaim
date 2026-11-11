import type { Dict } from './en';

const fr: Dict = {
  meta: {
    title: 'Reclaim | Récupérer des fichiers supprimés gratuitement sur Windows, macOS et Linux',
    description:
      'Récupérez photos, documents et autres fichiers supprimés sur clés USB, cartes SD et disques durs. Reclaim est gratuit, open source et analyse en lecture seule. NTFS, FAT32 et exFAT.',
    downloadTitle: 'Télécharger Reclaim | Récupération de fichiers gratuite pour Windows, macOS et Linux',
    downloadDescription:
      'Téléchargez le dernier installateur de Reclaim pour Windows, macOS (Apple Silicon et Intel) ou Debian et Ubuntu. Récupération de fichiers supprimés gratuite et open source.',
    notFoundTitle: 'Page introuvable | Reclaim',
    ogAlt: 'Reclaim, une application de bureau pour récupérer des fichiers supprimés',
  },
  nav: {
    skip: 'Aller au contenu',
    how: 'Fonctionnement',
    features: 'Fonctionnalités',
    formats: 'Types de fichiers',
    faq: 'FAQ',
    download: 'Télécharger',
    github: 'GitHub',
    language: 'Langue',
    home: 'Accueil Reclaim',
    menu: 'Menu',
  },
  hero: {
    eyebrow: 'Gratuit et open source · Windows, macOS, Linux',
    title: 'Récupérez vos fichiers supprimés sans aggraver la situation.',
    lead:
      'Reclaim analyse le disque en lecture seule, vous montre ce qu’il trouve et enregistre les fichiers choisis en lieu sûr. Photos, documents, archives et plus encore, depuis clés USB, cartes SD et disques durs.',
    cta: 'Télécharger Reclaim',
    ctaFor: 'Télécharger pour {os}',
    secondary: 'Tous les téléchargements',
    meta: 'Version {version}',
    released: 'Publiée le {date}',
    license: 'Licence MIT',
    shotDrives: 'Reclaim affichant la liste des disques à analyser, avec une clé USB sélectionnée',
    shotResults: 'Reclaim affichant des fichiers récupérables avec recherche et filtres par catégorie',
  },
  warn: {
    title: 'Fichiers perdus ? N’écrivez plus rien sur ce disque.',
    body:
      'Un fichier supprimé reste généralement sur le disque jusqu’à ce que de nouvelles données soient écrites par-dessus. Chaque enregistrement, téléchargement, installation ou mise à jour peut occuper cet espace.',
    external: {
      lead: 'Sur une carte mémoire, une clé USB ou un disque externe',
      text: 'Débranchez-le et ne prenez plus de photos avec. Si l’ordinateur ou l’appareil photo propose de le formater, refusez. Vous pouvez installer Reclaim normalement sur votre ordinateur.',
    },
    internal: {
      lead: 'Sur le disque principal de votre ordinateur',
      text: 'N’y installez rien. Téléchargez la version portable sur un autre ordinateur ou directement sur une clé USB, lancez Reclaim depuis la clé et enregistrez les fichiers récupérés sur un autre disque.',
    },
  },
  steps: {
    kicker: 'Fonctionnement',
    title: 'Quatre étapes entre « oh non » et « ouf ».',
    items: [
      {
        title: 'Choisissez un disque',
        body: 'Sélectionnez un disque, une clé USB, une carte SD ou une image disque. La liste se met à jour quand vous branchez un périphérique.',
      },
      {
        title: 'Analysez',
        body: 'Les résultats s’affichent pendant l’analyse. Arrêtez-la dès que vous voyez ce qu’il vous faut et continuez avec ce qui a été trouvé.',
      },
      {
        title: 'Vérifiez',
        body: 'Cherchez par nom ou par type, filtrez par catégorie, prévisualisez les images et consultez les détails de chaque fichier avant de décider.',
      },
      {
        title: 'Enregistrez ailleurs',
        body: 'Récupérez les fichiers sélectionnés dans un dossier situé sur un autre disque, pour ne pas écraser ceux qui restent à récupérer.',
      },
    ],
  },
  features: {
    kicker: 'Conçu pour une récupération prudente',
    title: 'Simple en surface. Prudent en profondeur.',
    items: [
      {
        title: 'Lecture seule',
        body: 'Reclaim n’écrit jamais sur le disque analysé. Il lit la table d’allocation du système de fichiers et ne cherche que dans l’espace libre, sans toucher aux fichiers existants.',
      },
      {
        title: 'Vérifiez avant d’enregistrer',
        body: 'Icônes par type, recherche, filtres par catégorie, aperçu des images et inspecteur de fichiers vous aident à trier les vraies trouvailles.',
      },
      {
        title: 'Compatible avec les disques courants',
        body: 'NTFS, FAT12/16/32 et exFAT : les formats des disques Windows, clés USB, cartes SD et cartes d’appareil photo.',
      },
      {
        title: 'Dans votre langue',
        body: 'Anglais, hébreu, arabe, espagnol, français, allemand et russe, avec mise en page de droite à gauche et thèmes clair et sombre.',
      },
      {
        title: 'Mises à jour sûres',
        body: 'Reclaim recherche les nouvelles versions et vérifie la signature et la somme de contrôle de chaque mise à jour avant de l’installer. Les données d’analyse ne quittent jamais votre ordinateur.',
      },
      {
        title: 'Gratuit et open source',
        body: 'Ni version d’essai, ni compte, ni option payante. Tout le code source est sur GitHub sous licence MIT.',
      },
    ],
  },
  formats: {
    kicker: 'Ce qu’il retrouve',
    title: 'Les fichiers que l’on perd le plus.',
    lead: 'Reclaim reconnaît les fichiers à leur contenu : il les retrouve même quand leur nom a disparu.',
    rows: [
      { name: 'Photos et images', list: 'JPEG, PNG, GIF, BMP, WebP, QOI, ICO' },
      { name: 'Documents et livres numériques', list: 'PDF, Word, Excel, PowerPoint, OpenDocument, EPUB' },
      { name: 'Archives', list: 'ZIP, 7z, RAR5, CAB, TAR, GZIP, XAR, JAR, APK' },
      { name: 'Programmes et installateurs', list: 'EXE, DLL, ELF, MSI, DEB, RPM, DEX, PKG' },
      { name: 'Audio et vidéo', list: 'WAV, AIFF, AU, AVI, Ogg Vorbis, Opus, Theora' },
      { name: 'Bases de données', list: 'SQLite' },
    ],
    more: 'Liste complète des formats',
  },
  limits: {
    kicker: 'Limites, en toute franchise',
    title: 'Ce que Reclaim ne sait pas (encore) faire.',
    lead: 'Reclaim est une préversion et la récupération n’est jamais garantie. Mieux vaut connaître ses limites avant de commencer.',
    items: [
      'Les données écrasées, ou effacées par le TRIM d’un SSD, ne peuvent pas être récupérées.',
      'Les noms et dossiers d’origine ne sont pas restaurés ; les fichiers reçoivent des noms générés.',
      'La plupart des fichiers sont limités à 64 Mio, et les fichiers fragmentés peuvent être incomplets.',
      'Les disques internes des Mac (APFS), les partitions système Linux (ext4, Btrfs) et les images de disque entier avec table de partitions ne peuvent pas encore être analysés.',
      'Les résultats ne sont pas conservés après fermeture : enregistrez d’abord ce dont vous avez besoin.',
    ],
  },
  faq: {
    title: 'Questions fréquentes',
    items: [
      {
        q: 'Reclaim est-il vraiment gratuit ?',
        a: 'Oui. Reclaim est gratuit et open source sous licence MIT. Pas d’offre payante, pas de limite de récupération, pas de compte.',
      },
      {
        q: 'Puis-je récupérer des fichiers après avoir vidé la corbeille ?',
        a: 'Si les fichiers sont encore dans la corbeille, restaurez-les depuis celle-ci : c’est plus rapide et ils gardent leur nom. Une fois la corbeille vidée, sur un disque dur, une carte mémoire ou une clé USB, les données restent en général sur le disque jusqu’à ce qu’elles soient écrasées : une analyse a donc de bonnes chances. Sur le SSD interne d’un ordinateur, elles sont souvent effacées peu après (voir la question sur les SSD).',
      },
      {
        q: 'Peut-il récupérer des photos sur une carte SD ou une clé USB ?',
        a: 'En général, oui : les cartes SD et la plupart des clés USB utilisent FAT32 ou exFAT, que Reclaim prend en charge. Arrêtez immédiatement d’utiliser la carte, ne prenez plus de photos et refusez toute proposition de formatage. Branchez-la ensuite avec un lecteur de cartes, choisissez-la dans Reclaim et lancez l’analyse.',
      },
      {
        q: 'L’analyse peut-elle modifier ou endommager mon disque ?',
        a: 'Non. Reclaim se contente de lire le disque analysé. Enregistrez les fichiers récupérés sur un autre disque : les enregistrer sur le même pourrait écraser ceux qui restent à récupérer.',
      },
      {
        q: 'Fonctionne-t-il avec les SSD ?',
        a: 'Les SSD internes conservent rarement longtemps les fichiers supprimés. Le système indique au SSD quel espace est libre (c’est le TRIM) et le SSD l’efface. Sous Windows et sur Mac, cela se produit généralement juste après la suppression ; sous Linux, cela attend souvent un nettoyage hebdomadaire. Une fois le TRIM appliqué, aucun logiciel ne peut récupérer les données. Les clés USB et de nombreux disques externes anciens ne reçoivent pas le TRIM, les chances y sont donc meilleures, mais les SSD externes récents le reçoivent souvent.',
      },
      {
        q: 'Et si le disque fait du bruit, est très lent ou se déconnecte ?',
        a: 'Arrêtez de l’utiliser et ne l’analysez pas. Un disque défaillant peut se dégrader à chaque lecture, et aucun logiciel de récupération ne répare le matériel. Si les données sont importantes, éteignez-le et faites appel à un service professionnel de récupération de données.',
      },
      {
        q: 'Pourquoi les fichiers récupérés ont-ils d’autres noms ?',
        a: 'Reclaim retrouve les fichiers d’après leur contenu dans l’espace libre, où le nom et le dossier d’origine ne sont généralement plus enregistrés. Ils sont enregistrés sous des noms générés : ouvrez-les pour vérifier leur contenu.',
      },
    ],
  },
  cta: {
    title: 'Plus vous analysez tôt, plus vos chances sont grandes.',
    body: 'Reclaim est gratuit et l’installateur pèse moins de 10 Mo.',
  },
  download: {
    title: 'Télécharger Reclaim',
    lead: 'Gratuit pour Windows, macOS et Linux. Chaque téléchargement est signé et accompagné d’une somme de contrôle SHA-256.',
    version: 'Dernière version : {version}',
    notes: 'Notes de version',
    recommended: 'Recommandé pour votre système',
    installer: 'Installateur',
    portable: 'ZIP portable',
    platforms: {
      windows: {
        name: 'Windows',
        detail: '64 bits (x86_64)',
        scans: 'Analyse les disques NTFS, FAT32 et exFAT : disques internes, cartes mémoire et clés USB.',
        how: 'Lancez l’installateur. Il installe Reclaim pour votre utilisateur, avec un raccourci dans le menu Démarrer et un désinstallateur. Si un disque ne s’ouvre pas, faites un clic droit sur Reclaim et choisissez « Exécuter en tant qu’administrateur ». Fichiers perdus sur le lecteur C: ? Utilisez plutôt la version portable depuis une clé USB.',
      },
      macos: {
        name: 'macOS',
        detail: 'Apple Silicon (M1 et suivants) ou Intel',
        scans: 'Analyse les cartes mémoire, clés USB et disques externes en FAT32, exFAT ou NTFS. Le disque interne du Mac (APFS) ne peut pas encore être analysé.',
        how: 'Ouvrez le paquet pour installer Reclaim dans Applications. L’app n’est pas encore notariée par Apple : si macOS la bloque, allez dans Réglages Système → Confidentialité et sécurité et cliquez sur « Ouvrir quand même ». La lecture d’un disque entier nécessite un mot de passe administrateur, et macOS peut aussi demander d’autoriser l’accès aux volumes amovibles.',
      },
      linux: {
        name: 'Linux',
        detail: 'Debian 12, Ubuntu 22.04 ou plus récent · x86_64',
        scans: 'Analyse les disques FAT32, exFAT et NTFS, comme les cartes mémoire et les clés USB. Les partitions système Linux (ext4, Btrfs) ne peuvent pas encore être analysées.',
        how: 'Dans le dossier de téléchargement, installez le paquet avec la commande ci-dessous. Quand vous choisissez un disque, Reclaim demande un mot de passe administrateur via l’invite habituelle du système. Les versions portables nécessitent glibc 2.35 ou plus récent.',
      },
    },
    arm: 'Apple Silicon',
    intel: 'Intel',
    deb: 'Paquet .deb',
    exe: 'Installateur Windows',
    copy: 'Copier',
    copied: 'Copié',
    verifyTitle: 'Vérifiez votre téléchargement',
    verifyBody:
      'Chaque fichier dispose d’une somme .sha256 et d’une signature .minisig sur la page de la version. Le système de mise à jour intégré de Reclaim vérifie les deux automatiquement.',
    allFiles: 'Tous les fichiers sur GitHub',
    sourceTitle: 'Compiler depuis les sources',
    sourceBody: 'Reclaim est écrit en Rust. Installez Rust stable et les outils de compilation de votre système, puis lancez :',
  },
  footer: {
    tagline: 'Récupération de fichiers supprimés, gratuite et open source.',
    product: 'Produit',
    project: 'Projet',
    source: 'Code source',
    releases: 'Versions',
    issues: 'Signaler un problème',
    docs: 'Guide du développeur',
    license: 'Publié sous licence MIT.',
  },
  notFound: {
    title: 'Cette page n’existe pas.',
    body: 'Contrairement à vos fichiers, celle-ci est irrécupérable. Essayez plutôt la page d’accueil.',
    back: 'Aller à l’accueil',
  },
  ui: {
    windowTitle: 'Reclaim - Récupération de fichiers',
    zoom: 'Agrandir la capture',
    close: 'Fermer',
    tour: 'Visite guidée',
  },
  os: { windows: 'Windows', macos: 'macOS', linux: 'Linux' },
};

export default fr;
