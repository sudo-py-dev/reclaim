#!/usr/bin/env python3
"""Build native installers from an already built, architecture-specific executable."""
import argparse
import os
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent.parent
VERSION = re.search(r'^version = "([^"]+)"', (ROOT / 'Cargo.toml').read_text(), re.M)[1]
IDENTIFIER = 'io.github.reclaim.Reclaim'


def run(*args):
    subprocess.run([str(arg) for arg in args], check=True)


def licenses(destination):
    destination.mkdir(parents=True, exist_ok=True)
    for source in ['LICENSE', 'assets/fonts/DejaVu-LICENSE.txt']:
        shutil.copy2(ROOT / source, destination)


def deb(binary, output, work):
    root = work / 'root'
    (root / 'DEBIAN').mkdir(parents=True)
    (root / 'usr/bin').mkdir(parents=True)
    shutil.copy2(binary, root / 'usr/bin/reclaim')
    (root / 'usr/bin/reclaim').chmod(0o755)
    applications = root / 'usr/share/applications'
    applications.mkdir(parents=True)
    shutil.copy2(ROOT / 'packaging/linux/io.github.reclaim.Reclaim.desktop', applications)
    icon_dir = root / 'usr/share/icons/hicolor/256x256/apps'
    icon_dir.mkdir(parents=True)
    shutil.copy2(ROOT / 'assets/icons/reclaim.png', icon_dir / f'{IDENTIFIER}.png')
    licenses(root / 'usr/share/doc/reclaim')
    (root / 'DEBIAN/control').write_text(f'''Package: reclaim
Version: {VERSION}
Section: utils
Priority: optional
Architecture: amd64
Maintainer: Reclaim contributors <sudo-py-dev@users.noreply.github.com>
Homepage: https://github.com/sudo-py-dev/reclaim
Depends: pkexec | policykit-1, libc6 (>= 2.35), libgcc-s1, libx11-6, libx11-xcb1, libxcb1, libxkbcommon0, libxkbcommon-x11-0, libwayland-client0, libwayland-cursor0, libegl1, libgl1, libxcursor1, libxi6, libxrandr2
Recommends: xdg-desktop-portal, xdg-desktop-portal-gtk
Installed-Size: {sum(p.stat().st_size for p in (root / 'usr').rglob('*') if p.is_file()) // 1024 + 1}
Description: Read-only disk image carving and file recovery
 Scan drives and disk images, review candidates, and recover selected files.
''')
    (root / 'usr/share/doc/reclaim/copyright').write_text(
        'Upstream-Name: Reclaim\nSource: https://github.com/sudo-py-dev/reclaim\n'
        'License: MIT; see LICENSE in this directory.\n'
        'Bundled fonts: see DejaVu-LICENSE.txt.\n')
    for path in root.rglob('*'):
        path.chmod(0o755 if path.is_dir() or path == root / 'usr/bin/reclaim' else 0o644)
    root.chmod(0o755)
    run('dpkg-deb', '--root-owner-group', '-Zxz', '-z9', '-Sextreme', '--build', root,
        output / f'reclaim_{VERSION}_amd64.deb')


def mac(binary, output, work, arch):
    package_root = work / 'root'
    app = package_root / 'Applications/Reclaim.app'
    contents = app / 'Contents'
    (contents / 'MacOS').mkdir(parents=True)
    executable = contents / 'MacOS/reclaim'
    shutil.copy2(binary, executable)
    executable.chmod(0o755)
    with (ROOT / 'packaging/macos/Info.plist').open('rb') as file:
        info = plistlib.load(file)
    info.update(CFBundleShortVersionString=VERSION, CFBundleVersion=VERSION)
    with (contents / 'Info.plist').open('wb') as file:
        plistlib.dump(info, file)
    licenses(contents / 'Resources/licenses')
    shutil.copy2(ROOT / 'assets/icons/reclaim.icns', contents / 'Resources/reclaim.icns')
    run('codesign', '--force', '--sign', '-', app)
    run('codesign', '--verify', '--deep', '--strict', app)
    components = work / 'components.plist'
    run('pkgbuild', '--analyze', '--root', package_root, components)
    with components.open('rb') as file:
        configuration = plistlib.load(file)
    for component in configuration:
        component['BundleIsRelocatable'] = False
    with components.open('wb') as file:
        plistlib.dump(configuration, file)
    # A native Installer package installs the app into Applications, with a stable receipt.
    run('pkgbuild', '--root', package_root, '--component-plist', components, '--install-location', '/',
        '--identifier', IDENTIFIER, '--version', VERSION,
        output / f'reclaim-macos-{arch}.pkg')


def windows(binary, output):
    compiler = shutil.which('ISCC.exe') or shutil.which('iscc')
    if not compiler:
        candidates = sorted(Path(os.environ.get('ProgramFiles(x86)', r'C:\Program Files (x86)')).glob('Inno Setup */ISCC.exe'))
        if not candidates:
            raise SystemExit('Install Inno Setup 6 or newer, or add ISCC.exe to PATH')
        compiler = str(candidates[-1])
    run(compiler, f'/DAppVersion={VERSION}', f'/DBinaryPath={binary}',
        f'/DOutputPath={output}', ROOT / 'packaging/windows/reclaim.iss')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('platform', choices=['linux', 'windows', 'macos'])
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, default=ROOT / 'dist')
    parser.add_argument('--arch', choices=['x86_64', 'aarch64'], default='x86_64')
    args = parser.parse_args()
    binary, output = args.binary.resolve(strict=True), args.output.resolve()
    if args.platform != 'macos' and args.arch != 'x86_64':
        parser.error('Windows and Debian installers currently support x86_64 only')
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='reclaim-package-') as temporary:
        work = Path(temporary)
        if args.platform == 'linux':
            deb(binary, output, work)
        elif args.platform == 'macos':
            mac(binary, output, work, args.arch)
        else:
            windows(binary, output)


if __name__ == '__main__':
    main()
