import type { Dict } from './en';

const es: Dict = {
  meta: {
    title: 'Reclaim | Recuperar archivos borrados gratis en Windows, macOS y Linux',
    description:
      'Recupera fotos, documentos y otros archivos borrados de memorias USB, tarjetas SD y discos duros. Reclaim es gratuito, de código abierto y solo lee el disco. NTFS, FAT32 y exFAT.',
    downloadTitle: 'Descargar Reclaim | Recuperación de archivos gratis para Windows, macOS y Linux',
    downloadDescription:
      'Descarga el instalador más reciente de Reclaim para Windows, macOS (Apple Silicon e Intel) o Debian y Ubuntu. Recuperación de archivos borrados gratuita y de código abierto.',
    notFoundTitle: 'Página no encontrada | Reclaim',
    ogAlt: 'Reclaim, una aplicación de escritorio para recuperar archivos borrados',
  },
  nav: {
    skip: 'Saltar al contenido',
    how: 'Cómo funciona',
    features: 'Funciones',
    formats: 'Tipos de archivo',
    faq: 'Preguntas',
    download: 'Descargar',
    github: 'GitHub',
    language: 'Idioma',
    home: 'Inicio de Reclaim',
    menu: 'Menú',
  },
  hero: {
    eyebrow: 'Gratis y de código abierto · Windows, macOS, Linux',
    title: 'Recupera archivos borrados sin empeorar las cosas.',
    lead:
      'Reclaim analiza la unidad en modo de solo lectura, te muestra lo que encuentra y guarda los archivos que elijas en un lugar seguro. Fotos, documentos, archivos comprimidos y más, desde memorias USB, tarjetas SD y discos duros.',
    cta: 'Descargar Reclaim',
    ctaFor: 'Descargar para {os}',
    secondary: 'Todas las descargas',
    meta: 'Versión {version}',
    released: 'Publicada el {date}',
    license: 'Licencia MIT',
    shotDrives: 'Reclaim mostrando una lista de unidades para analizar, con una memoria USB seleccionada',
    shotResults: 'Reclaim mostrando archivos recuperables con búsqueda y filtros por categoría',
  },
  warn: {
    title: '¿Has perdido archivos? Deja de escribir en esa unidad.',
    body:
      'Un archivo borrado suele seguir en la unidad hasta que se escriben datos nuevos encima. Cada archivo guardado, descarga, instalación o actualización puede ocupar ese espacio.',
    external: {
      lead: 'En una tarjeta de memoria, memoria USB o disco externo',
      text: 'Desconéctala y no hagas más fotos con ella. Si el ordenador o la cámara te ofrecen formatearla, di que no. Puedes instalar Reclaim en tu ordenador con normalidad.',
    },
    internal: {
      lead: 'En el disco principal de tu ordenador',
      text: 'No instales nada en él. Descarga la versión portátil en otro ordenador o guárdala directamente en una memoria USB, ejecuta Reclaim desde ahí y guarda los archivos recuperados en otra unidad.',
    },
  },
  steps: {
    kicker: 'Cómo funciona',
    title: 'Cuatro pasos de «¡oh, no!» a «¡aquí está!».',
    items: [
      {
        title: 'Elige una unidad',
        body: 'Selecciona un disco, una memoria USB, una tarjeta SD o una imagen de disco. La lista se actualiza al conectar dispositivos.',
      },
      {
        title: 'Analiza',
        body: 'Los resultados aparecen mientras avanza el análisis. Puedes detenerlo en cuanto veas lo que necesitas y seguir trabajando con lo encontrado.',
      },
      {
        title: 'Revisa',
        body: 'Busca por nombre o tipo, filtra por categoría, previsualiza imágenes y consulta los detalles de cada archivo antes de decidir.',
      },
      {
        title: 'Guarda en otro sitio',
        body: 'Recupera los archivos seleccionados en una carpeta de otra unidad, para no sobrescribir los que aún no has recuperado.',
      },
    ],
  },
  features: {
    kicker: 'Pensado para recuperar con cuidado',
    title: 'Sencillo por fuera. Prudente por dentro.',
    items: [
      {
        title: 'Solo lectura',
        body: 'Reclaim nunca escribe en la unidad que analiza. Lee el mapa de asignación del sistema de archivos y solo busca en el espacio libre, sin tocar los archivos existentes.',
      },
      {
        title: 'Revisa antes de guardar',
        body: 'Iconos por tipo, búsqueda, filtros por categoría, vista previa de imágenes y un inspector de archivos te ayudan a separar lo útil del ruido.',
      },
      {
        title: 'Compatible con las unidades habituales',
        body: 'NTFS, FAT12/16/32 y exFAT: los formatos de discos de Windows, memorias USB, tarjetas SD y tarjetas de cámara.',
      },
      {
        title: 'En tu idioma',
        body: 'Inglés, hebreo, árabe, español, francés, alemán y ruso, con diseño de derecha a izquierda y temas claro y oscuro.',
      },
      {
        title: 'Actualizaciones seguras',
        body: 'Reclaim busca nuevas versiones y verifica la firma y la suma de comprobación de cada actualización antes de instalarla. Los datos del análisis nunca salen de tu equipo.',
      },
      {
        title: 'Gratis y de código abierto',
        body: 'Sin versión de prueba, sin cuenta y sin ventas adicionales. Todo el código está en GitHub bajo licencia MIT.',
      },
    ],
  },
  formats: {
    kicker: 'Qué encuentra',
    title: 'Los archivos que más se pierden.',
    lead: 'Reclaim reconoce los archivos por su contenido, así que puede encontrarlos aunque se haya perdido el nombre.',
    rows: [
      { name: 'Fotos e imágenes', list: 'JPEG, PNG, GIF, BMP, WebP, QOI, ICO' },
      { name: 'Documentos y libros electrónicos', list: 'PDF, Word, Excel, PowerPoint, OpenDocument, EPUB' },
      { name: 'Archivos comprimidos', list: 'ZIP, 7z, RAR5, CAB, TAR, GZIP, XAR, JAR, APK' },
      { name: 'Programas e instaladores', list: 'EXE, DLL, ELF, MSI, DEB, RPM, DEX, PKG' },
      { name: 'Audio y vídeo', list: 'WAV, AIFF, AU, AVI, Ogg Vorbis, Opus, Theora' },
      { name: 'Bases de datos', list: 'SQLite' },
    ],
    more: 'Cobertura completa de formatos',
  },
  limits: {
    kicker: 'Límites, con franqueza',
    title: 'Lo que Reclaim (todavía) no puede hacer.',
    lead: 'Reclaim es una versión preliminar y la recuperación nunca está garantizada. Conviene conocer sus límites antes de empezar.',
    items: [
      'Los datos sobrescritos, o borrados por el TRIM de un SSD, no se pueden recuperar.',
      'No se restauran los nombres ni las carpetas originales; los archivos reciben nombres generados.',
      'La mayoría de los archivos están limitados a 64 MiB, y los fragmentados pueden quedar incompletos.',
      'Aún no se pueden analizar los discos internos de Mac (APFS), las particiones de sistema de Linux (ext4, Btrfs) ni las imágenes de disco completo con tabla de particiones.',
      'Los resultados no se conservan al cerrar la aplicación: guarda primero lo que necesites.',
    ],
  },
  faq: {
    title: 'Preguntas frecuentes',
    items: [
      {
        q: '¿Reclaim es realmente gratis?',
        a: 'Sí. Reclaim es gratuito y de código abierto bajo licencia MIT. No hay planes de pago, límites de recuperación ni cuentas.',
      },
      {
        q: '¿Puedo recuperar archivos después de vaciar la papelera?',
        a: 'Si los archivos siguen en la papelera, restáuralos desde ahí: es más rápido y conservan su nombre. Una vez vaciada, en discos duros, tarjetas de memoria y memorias USB los datos suelen seguir en el disco hasta que algo los sobrescribe, así que un análisis tiene buenas posibilidades. En el SSD interno de un ordenador se suelen borrar poco después (consulta la pregunta sobre SSD).',
      },
      {
        q: '¿Puede recuperar fotos de una tarjeta SD o una memoria USB?',
        a: 'Normalmente, sí: las tarjetas SD y la mayoría de las memorias USB usan FAT32 o exFAT, que Reclaim admite. Deja de usar la tarjeta de inmediato, no hagas más fotos y rechaza cualquier oferta de formatearla. Después conéctala con un lector de tarjetas, elígela en Reclaim y empieza el análisis.',
      },
      {
        q: '¿El análisis puede modificar o dañar mi unidad?',
        a: 'No. Reclaim solo lee la unidad que analiza. Guarda los archivos recuperados en otra unidad: guardarlos en la misma podría sobrescribir los que aún no has recuperado.',
      },
      {
        q: '¿Funciona con discos SSD?',
        a: 'Los SSD internos no suelen conservar mucho tiempo los archivos borrados. El sistema indica al SSD qué espacio está libre (es lo que se llama TRIM) y el SSD lo borra. En Windows y Mac esto suele ocurrir justo después de borrar; en Linux a menudo espera a una limpieza semanal. Una vez aplicado, ningún programa puede recuperar los datos. Las memorias USB y muchos discos externos antiguos no reciben TRIM, así que en ellos hay más posibilidades, aunque los SSD externos modernos a menudo sí lo reciben.',
      },
      {
        q: '¿Y si la unidad hace ruidos, va muy lenta o se desconecta?',
        a: 'Deja de usarla y no la analices. Una unidad que falla puede empeorar con cada lectura, y ningún programa de recuperación arregla el hardware. Si los datos son importantes, apágala y acude a un servicio profesional de recuperación de datos.',
      },
      {
        q: '¿Por qué los archivos recuperados tienen otros nombres?',
        a: 'Reclaim encuentra los archivos por su contenido en el espacio libre, donde normalmente ya no constan el nombre ni la carpeta originales. Se guardan con nombres generados, así que ábrelos para comprobar qué son.',
      },
    ],
  },
  cta: {
    title: 'Cuanto antes analices, más posibilidades tendrás.',
    body: 'Reclaim es gratuito y el instalador ocupa menos de 10 MB.',
  },
  download: {
    title: 'Descargar Reclaim',
    lead: 'Gratis para Windows, macOS y Linux. Cada descarga está firmada e incluye una suma de comprobación SHA-256.',
    version: 'Última versión: {version}',
    notes: 'Notas de la versión',
    recommended: 'Recomendado para tu sistema',
    installer: 'Instalador',
    portable: 'ZIP portátil',
    platforms: {
      windows: {
        name: 'Windows',
        detail: '64 bits (x86_64)',
        scans: 'Analiza unidades NTFS, FAT32 y exFAT: discos internos, tarjetas de memoria y memorias USB.',
        how: 'Ejecuta el instalador. Se instala para tu usuario y añade un acceso directo al menú Inicio y un desinstalador. Si una unidad no se abre, haz clic derecho en Reclaim y elige «Ejecutar como administrador». ¿Has perdido archivos en la unidad C:? Usa la versión portátil desde una memoria USB.',
      },
      macos: {
        name: 'macOS',
        detail: 'Apple Silicon (M1 o posterior) o Intel',
        scans: 'Analiza tarjetas de memoria, memorias USB y discos externos en FAT32, exFAT o NTFS. El disco interno del Mac (APFS) aún no se puede analizar.',
        how: 'Abre el paquete para instalar Reclaim en Aplicaciones. Aún no está notarizada por Apple: si macOS la bloquea, ve a Ajustes del Sistema → Privacidad y seguridad y pulsa «Abrir igualmente». Leer una unidad completa requiere la contraseña de administrador, y macOS también puede pedirte que permitas el acceso a volúmenes extraíbles.',
      },
      linux: {
        name: 'Linux',
        detail: 'Debian 12, Ubuntu 22.04 o posterior · x86_64',
        scans: 'Analiza unidades FAT32, exFAT y NTFS, como tarjetas de memoria y memorias USB. Las particiones de sistema de Linux (ext4, Btrfs) aún no se pueden analizar.',
        how: 'En la carpeta donde lo descargaste, instala el paquete con el siguiente comando. Al elegir una unidad, Reclaim te pedirá una contraseña de administrador mediante el aviso habitual del sistema. Las versiones portátiles requieren glibc 2.35 o posterior.',
      },
    },
    arm: 'Apple Silicon',
    intel: 'Intel',
    deb: 'Paquete .deb',
    exe: 'Instalador de Windows',
    copy: 'Copiar',
    copied: 'Copiado',
    verifyTitle: 'Verifica la descarga',
    verifyBody:
      'Cada archivo tiene su suma .sha256 y su firma .minisig en la página de la versión. El actualizador integrado de Reclaim comprueba ambas automáticamente.',
    allFiles: 'Todos los archivos en GitHub',
    sourceTitle: 'Compilar desde el código fuente',
    sourceBody: 'Reclaim está escrito en Rust. Instala Rust estable y las herramientas de compilación de tu sistema, y ejecuta:',
  },
  footer: {
    tagline: 'Recuperación de archivos borrados, gratuita y de código abierto.',
    product: 'Producto',
    project: 'Proyecto',
    source: 'Código fuente',
    releases: 'Versiones',
    issues: 'Informar de un problema',
    docs: 'Guía para desarrolladores',
    license: 'Publicado bajo licencia MIT.',
  },
  notFound: {
    title: 'Esta página no existe.',
    body: 'A diferencia de tus archivos, esta no se puede recuperar. Prueba con la página de inicio.',
    back: 'Ir a la página de inicio',
  },
  ui: {
    windowTitle: 'Reclaim - Recuperación de archivos',
    zoom: 'Ampliar captura',
    close: 'Cerrar',
    tour: 'Recorrido por la app',
  },
  os: { windows: 'Windows', macos: 'macOS', linux: 'Linux' },
};

export default es;
