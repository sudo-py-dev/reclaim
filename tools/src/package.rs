//! Native installer assembly from an already built executable.
use crate::{IDENTIFIER, Result, copy, files, root, run, version};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn permissions(path: &Path, mode: u32) -> Result {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    }
    #[cfg(not(unix))]
    let _ = (path, mode);
    Ok(())
}
fn licenses(destination: &Path) -> Result {
    for source in ["LICENSE", "assets/fonts/DejaVu-LICENSE.txt"] {
        copy(
            root().join(source),
            destination.join(Path::new(source).file_name().ok_or("license has no name")?),
        )?;
    }
    Ok(())
}
fn deb_tree(binary: &Path, tree: &Path) -> Result {
    fs::create_dir_all(tree.join("DEBIAN"))?;
    copy(binary, tree.join("usr/bin/reclaim"))?;
    copy(
        root().join("packaging/linux/io.github.reclaim.Reclaim.desktop"),
        tree.join(format!("usr/share/applications/{IDENTIFIER}.desktop")),
    )?;
    copy(
        root().join("assets/icons/reclaim.png"),
        tree.join(format!(
            "usr/share/icons/hicolor/256x256/apps/{IDENTIFIER}.png"
        )),
    )?;
    licenses(&tree.join("usr/share/doc/reclaim"))?;
    fs::write(
        tree.join("usr/share/doc/reclaim/copyright"),
        "Upstream-Name: Reclaim\nSource: https://github.com/sudo-py-dev/reclaim\nLicense: MIT; see LICENSE in this directory.\nBundled fonts: see DejaVu-LICENSE.txt.\n",
    )?;
    let size = files(&tree.join("usr"))?
        .into_iter()
        .try_fold(0u64, |total, path| {
            fs::metadata(path).map(|m| total + m.len())
        })?;
    fs::write(
        tree.join("DEBIAN/control"),
        format!(
            "Package: reclaim\nVersion: {}\nSection: utils\nPriority: optional\nArchitecture: amd64\nMaintainer: Reclaim contributors <sudo-py-dev@users.noreply.github.com>\nHomepage: https://github.com/sudo-py-dev/reclaim\nDepends: pkexec | policykit-1, libc6 (>= 2.35), libgcc-s1, libx11-6, libx11-xcb1, libxcb1, libxkbcommon0, libxkbcommon-x11-0, libwayland-client0, libwayland-cursor0, libegl1, libgl1, libxcursor1, libxi6, libxrandr2\nRecommends: xdg-desktop-portal, xdg-desktop-portal-gtk\nInstalled-Size: {}\nDescription: Read-only disk image carving and file recovery\n Scan drives and disk images, review candidates, and recover selected files.\n",
            version()?,
            size / 1024 + 1
        ),
    )?;
    fn set_tree(path: &Path) -> Result {
        permissions(path, if path.is_dir() { 0o755 } else { 0o644 })?;
        if path.is_dir() {
            for entry in fs::read_dir(path)? {
                set_tree(&entry?.path())?;
            }
        }
        Ok(())
    }
    set_tree(tree)?;
    permissions(&tree.join("usr/bin/reclaim"), 0o755)
}
fn deb(binary: &Path, output: &Path, work: &Path) -> Result {
    let tree = work.join("root");
    deb_tree(binary, &tree)?;
    run(Command::new("dpkg-deb")
        .args(["--root-owner-group", "-Zxz", "-z9", "-Sextreme", "--build"])
        .arg(tree)
        .arg(output.join(format!("reclaim_{}_amd64.deb", version()?))))
}
fn mac(binary: &Path, output: &Path, work: &Path, arch: &str) -> Result {
    let tree = work.join("root");
    let app = tree.join("Applications/Reclaim.app");
    let contents = app.join("Contents");
    copy(binary, contents.join("MacOS/reclaim"))?;
    permissions(&contents.join("MacOS/reclaim"), 0o755)?;
    let info = contents.join("Info.plist");
    copy(root().join("packaging/macos/Info.plist"), &info)?;
    let version = version()?;
    for key in ["CFBundleShortVersionString", "CFBundleVersion"] {
        run(Command::new("plutil")
            .args(["-replace", key, "-string", &version])
            .arg(&info))?;
    }
    licenses(&contents.join("Resources/licenses"))?;
    copy(
        root().join("assets/icons/reclaim.icns"),
        contents.join("Resources/reclaim.icns"),
    )?;
    run(Command::new("codesign")
        .args(["--force", "--sign", "-"])
        .arg(&app))?;
    run(Command::new("codesign")
        .args(["--verify", "--deep", "--strict"])
        .arg(&app))?;
    // A fixed bundle path avoids accidentally relocating an existing installation.
    let components = work.join("components.plist");
    fs::write(
        &components,
        "<?xml version=\"1.0\"?><plist version=\"1.0\"><array><dict><key>RootRelativeBundlePath</key><string>Applications/Reclaim.app</string><key>BundleIsRelocatable</key><false/><key>BundleHasStrictIdentifier</key><true/><key>BundleIsVersionChecked</key><true/><key>BundleOverwriteAction</key><string>upgrade</string></dict></array></plist>",
    )?;
    run(Command::new("pkgbuild")
        .arg("--root")
        .arg(tree)
        .arg("--component-plist")
        .arg(components)
        .args([
            "--install-location",
            "/",
            "--identifier",
            IDENTIFIER,
            "--version",
            &version,
        ])
        .arg(output.join(format!("reclaim-macos-{arch}.pkg"))))
}
/// Velopack package id: names the install folder (`%LocalAppData%\\<id>`) and
/// the uninstall entry. Matches the WinGet identifier.
const VELOPACK_ID: &str = "sudo-py-dev.Reclaim";
/// Release asset names. The updater (`src/updater.rs`) downloads the package;
/// the setup is only for first installs and WinGet.
const WINDOWS_SETUP: &str = "reclaim-windows-x86_64-setup.exe";
const WINDOWS_PACKAGE: &str = "reclaim-windows-x86_64-full.nupkg";

/// The files Velopack installs into `current\\`: the app and its licenses.
fn velopack_tree(binary: &Path, tree: &Path) -> Result {
    copy(binary, tree.join("reclaim.exe"))?;
    licenses(&tree.join("licenses"))
}

/// Move vpk's version-stamped outputs to the fixed release asset names.
fn velopack_outputs(packed: &Path, output: &Path) -> Result {
    for (suffix, name) in [
        ("-Setup.exe", WINDOWS_SETUP),
        ("-full.nupkg", WINDOWS_PACKAGE),
    ] {
        let mut found = fs::read_dir(packed)?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .is_some_and(|n| n.to_string_lossy().ends_with(suffix))
            });
        let source = found
            .next()
            .ok_or_else(|| format!("vpk produced no *{suffix}"))?;
        if found.next().is_some() {
            return Err(format!("vpk produced more than one *{suffix}").into());
        }
        let target = output.join(name);
        if target.exists() {
            fs::remove_file(&target)?;
        }
        copy(&source, &target)?;
    }
    Ok(())
}

fn windows(binary: &Path, output: &Path, work: &Path) -> Result {
    let tree = work.join("app");
    let packed = work.join("velopack");
    velopack_tree(binary, &tree)?;
    run(Command::new("vpk")
        .args([
            "pack",
            "--packId",
            VELOPACK_ID,
            "--packVersion",
            &version()?,
        ])
        .args([
            "--packTitle",
            "Reclaim",
            "--packAuthors",
            "Reclaim contributors",
        ])
        .args([
            "--mainExe",
            "reclaim.exe",
            "--runtime",
            "win-x64",
            "--channel",
            "win",
        ])
        .args([
            "--shortcuts",
            "StartMenuRoot",
            "--delta",
            "None",
            "--noPortable",
        ])
        .arg("--icon")
        .arg(root().join("assets/icons/reclaim.ico"))
        .arg("--packDir")
        .arg(&tree)
        .arg("--outputDir")
        .arg(&packed))?;
    velopack_outputs(&packed, output)
}
pub fn main(args: &[String]) -> Result {
    let platform = args
        .first()
        .ok_or("package requires linux, windows, or macos")?;
    let mut binary = None;
    let mut output = root().join("dist");
    let mut arch = "x86_64";
    let (options, remainder) = args[1..].as_chunks::<2>();
    for pair in options {
        match pair[0].as_str() {
            "--binary" => binary = Some(PathBuf::from(&pair[1])),
            "--output" => output = PathBuf::from(&pair[1]),
            "--arch" => arch = &pair[1],
            _ => return Err(format!("unknown package option: {}", pair[0]).into()),
        }
    }
    if !remainder.is_empty() {
        return Err("package option needs a value".into());
    }
    if !matches!(arch, "x86_64" | "aarch64") || (platform != "macos" && arch != "x86_64") {
        return Err("only macOS supports aarch64 installers".into());
    }
    if !matches!(
        (platform.as_str(), std::env::consts::OS),
        ("linux", "linux") | ("windows", "windows") | ("macos", "macos")
    ) {
        return Err("build installers on their native operating system".into());
    }
    // Keep ordinary Windows paths: canonicalize() adds an extended-length
    // prefix that some native installer tools cannot consume.
    let binary = std::path::absolute(binary.ok_or("--binary is required")?)?;
    if !binary.is_file() {
        return Err("--binary must name an executable file".into());
    }
    fs::create_dir_all(&output)?;
    let output = std::path::absolute(output)?;
    let work = tempfile::Builder::new()
        .prefix("reclaim-package-")
        .tempdir()?;
    match platform.as_str() {
        "linux" => deb(&binary, &output, work.path()),
        "macos" => mac(&binary, &output, work.path(), arch),
        "windows" => windows(&binary, &output, work.path()),
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn debian_tree_contains_desktop_integration_and_licenses() -> Result {
        let work = tempfile::tempdir()?;
        let binary = work.path().join("binary");
        fs::write(&binary, b"installer input")?;
        let tree = work.path().join("tree");
        deb_tree(&binary, &tree)?;
        assert_eq!(fs::read(tree.join("usr/bin/reclaim"))?, b"installer input");
        assert!(
            tree.join(format!("usr/share/applications/{IDENTIFIER}.desktop"))
                .is_file()
        );
        assert!(
            tree.join("usr/share/doc/reclaim/DejaVu-LICENSE.txt")
                .is_file()
        );
        let control = fs::read_to_string(tree.join("DEBIAN/control"))?;
        assert!(control.contains(&format!("Version: {}\n", version()?)));
        assert!(control.contains("Architecture: amd64\n"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(tree.join("usr/bin/reclaim"))?
                    .permissions()
                    .mode()
                    & 0o777,
                0o755
            );
            assert_eq!(
                fs::metadata(tree.join("DEBIAN/control"))?
                    .permissions()
                    .mode()
                    & 0o777,
                0o644
            );
        }
        Ok(())
    }

    #[test]
    fn velopack_tree_and_outputs_use_fixed_release_names() -> Result {
        let work = tempfile::tempdir()?;
        let binary = work.path().join("binary");
        fs::write(&binary, b"installer input")?;
        let tree = work.path().join("tree");
        velopack_tree(&binary, &tree)?;
        assert_eq!(fs::read(tree.join("reclaim.exe"))?, b"installer input");
        assert!(tree.join("licenses/LICENSE").is_file());
        assert!(tree.join("licenses/DejaVu-LICENSE.txt").is_file());

        let packed = work.path().join("packed");
        fs::create_dir(&packed)?;
        let output = work.path().join("dist");
        fs::create_dir(&output)?;
        fs::write(packed.join("sudo-py-dev.Reclaim-win-Setup.exe"), b"setup")?;
        assert!(velopack_outputs(&packed, &output).is_err());
        fs::write(
            packed.join("sudo-py-dev.Reclaim-1.0.0-full.nupkg"),
            b"package",
        )?;
        fs::write(packed.join("releases.win.json"), b"{}")?;
        velopack_outputs(&packed, &output)?;
        assert_eq!(fs::read(output.join(WINDOWS_SETUP))?, b"setup");
        assert_eq!(fs::read(output.join(WINDOWS_PACKAGE))?, b"package");
        assert_eq!(fs::read_dir(&output)?.count(), 2);
        Ok(())
    }
}
