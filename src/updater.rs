//! Portable executable updates from this project's stable GitHub releases.
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    time::Duration,
};

const REPO: &str = "sudo-py-dev/reclaim";
const MAX_BINARY: u64 = 128 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct Release {
    pub version: semver::Version,
    pub notes: String,
    pub url: String,
    pub digest: String,
    pub size: u64,
}

pub fn asset_name() -> String {
    format!(
        "reclaim-{}-{}{}",
        std::env::consts::OS,
        std::env::consts::ARCH,
        if cfg!(windows) { ".exe" } else { "" }
    )
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(180)))
        .timeout_connect(Some(Duration::from_secs(15)))
        .https_only(true)
        .build()
        .into()
}

pub fn check() -> Result<Option<Release>, String> {
    let mut response = agent()
        .get(&format!(
            "https://api.github.com/repos/{REPO}/releases/latest"
        ))
        .header("User-Agent", concat!("Reclaim/", env!("CARGO_PKG_VERSION")))
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("Could not check GitHub releases: {e}"))?;
    let json = response
        .body_mut()
        .with_config()
        .limit(2 * 1024 * 1024)
        .read_to_string()
        .map_err(|e| e.to_string())?;
    parse_release(
        &serde_json::from_str(&json).map_err(|e| e.to_string())?,
        &asset_name(),
    )
}

fn parse_release(json: &Value, asset_name: &str) -> Result<Option<Release>, String> {
    if json["draft"].as_bool() != Some(false) || json["prerelease"].as_bool() != Some(false) {
        return Ok(None);
    }
    let tag = json["tag_name"].as_str().ok_or("Missing release version")?;
    let version = semver::Version::parse(tag.trim_start_matches('v')).map_err(|e| e.to_string())?;
    if !version.pre.is_empty()
        || version <= semver::Version::parse(env!("CARGO_PKG_VERSION")).unwrap()
    {
        return Ok(None);
    }
    let asset = json["assets"]
        .as_array()
        .ok_or("Missing release assets")?
        .iter()
        .find(|a| a["name"].as_str() == Some(asset_name))
        .ok_or("No update for this platform yet")?;
    let url = asset["browser_download_url"]
        .as_str()
        .ok_or("Missing download URL")?;
    if url != format!("https://github.com/{REPO}/releases/download/{tag}/{asset_name}")
        || !(tag == version.to_string() || tag == format!("v{version}"))
    {
        return Err("Unexpected release download URL".into());
    }
    let digest = asset["digest"]
        .as_str()
        .and_then(|s| s.strip_prefix("sha256:"))
        .filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or("Release has no valid SHA-256 digest")?
        .to_ascii_lowercase();
    let size = asset["size"]
        .as_u64()
        .filter(|s| *s > 0 && *s <= MAX_BINARY)
        .ok_or("Invalid update size")?;
    Ok(Some(Release {
        version,
        notes: json["body"].as_str().unwrap_or("No release notes.").into(),
        url: url.into(),
        digest,
        size,
    }))
}

/// Installed packages must be updated as packages to preserve receipts and uninstall metadata.
pub fn installer_download(release: &Release) -> Option<String> {
    let executable = std::env::current_exe().ok()?;
    let name = installer_name(&executable, &release.version)?;
    Some(format!("{}/{}", release.url.rsplit_once('/')?.0, name))
}

fn installer_name(executable: &std::path::Path, version: &semver::Version) -> Option<String> {
    if cfg!(target_os = "linux")
        && matches!(
            executable.to_str(),
            Some("/usr/bin/reclaim" | "/bin/reclaim")
        )
    {
        Some(format!("reclaim_{version}_amd64.deb"))
    } else if cfg!(target_os = "macos")
        && executable
            .ancestors()
            .any(|p| p.extension().is_some_and(|e| e == "app"))
    {
        Some(format!("reclaim-macos-{}.pkg", std::env::consts::ARCH))
    } else if cfg!(windows) {
        Some("reclaim-windows-x86_64-setup.exe".into())
    } else {
        None
    }
}

pub fn install(release: &Release) -> Result<(), String> {
    if installer_download(release).is_some() {
        return Err("This installation must be updated using the matching installer.".into());
    }
    if release.url
        != format!(
            "https://github.com/{REPO}/releases/download/v{}/{}",
            release.version,
            asset_name()
        )
        && release.url
            != format!(
                "https://github.com/{REPO}/releases/download/{}/{}",
                release.version,
                asset_name()
            )
    {
        return Err("Unexpected release download URL".into());
    }
    if cfg!(windows) {
        return Err("This installation must be updated using the matching installer.".into());
    }
    // Private temporary file, bounded streaming download, verify before replacing anything.
    let mut file = tempfile::NamedTempFile::new().map_err(|e| e.to_string())?;
    let mut response = agent()
        .get(&release.url)
        .call()
        .map_err(|e| e.to_string())?;
    verify_download(response.body_mut().as_reader(), &mut file, release)?;
    file.as_file().sync_all().map_err(|e| e.to_string())?;
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    replace_executable(&executable, file.path()).map_err(|e| {
        format!("Could not install update: {e}. Move Reclaim to a writable folder and retry.")
    })
}

// Unix permits replacing a running executable. Publish a synced file on the
// same filesystem using Rust filesystem APIs; never spawn a cleanup helper.
fn replace_executable(
    executable: &std::path::Path,
    downloaded: &std::path::Path,
) -> std::io::Result<()> {
    let permissions = executable.metadata()?.permissions();
    let parent = executable
        .parent()
        .ok_or_else(|| std::io::Error::other("Executable has no parent"))?;
    let mut pending = tempfile::NamedTempFile::new_in(parent)?;
    std::io::copy(&mut std::fs::File::open(downloaded)?, &mut pending)?;
    pending.as_file().set_permissions(permissions)?;
    pending.as_file().sync_all()?;
    pending.persist(executable).map_err(|e| e.error)?;
    Ok(())
}

fn verify_download(
    reader: impl Read,
    mut file: impl Write,
    release: &Release,
) -> Result<(), String> {
    // Release is public: enforce bounds here as well as when parsing the API.
    if release.size == 0
        || release.size > MAX_BINARY
        || release.digest.len() != 64
        || !release.digest.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("Invalid update metadata".into());
    }
    let mut reader = reader.take(release.size + 1);
    let mut hash = Sha256::new();
    let mut size = 0;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = reader.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        size += count as u64;
        if size > release.size {
            return Err("Update exceeds expected size".into());
        }
        hash.update(&buffer[..count]);
        file.write_all(&buffer[..count])
            .map_err(|e| e.to_string())?;
    }
    if size != release.size || format!("{:x}", hash.finalize()) != release.digest {
        return Err("Update checksum verification failed".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn release() -> Value {
        serde_json::json!({"draft":false,"prerelease":false,"tag_name":"v99.0.0",
            "body":"What's new", "assets":[{"name":"test", "size":123,
            "digest":format!("sha256:{}", "a".repeat(64)),
            "browser_download_url":"https://github.com/sudo-py-dev/reclaim/releases/download/v99.0.0/test"}]})
    }
    #[test]
    fn replacement_uses_filesystem_apis_and_preserves_permissions() {
        let dir = tempfile::tempdir().unwrap();
        let installed = dir.path().join("reclaim");
        let downloaded = dir.path().join("download");
        std::fs::write(&installed, b"old").unwrap();
        std::fs::write(&downloaded, b"new").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&installed, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        replace_executable(&installed, &downloaded).unwrap();
        assert_eq!(std::fs::read(&installed).unwrap(), b"new");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                installed.metadata().unwrap().permissions().mode() & 0o777,
                0o755
            );
        }
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[test]
    fn portable_and_packaged_update_paths() {
        let version = semver::Version::parse("1.2.3").unwrap();
        #[cfg(not(windows))]
        assert!(installer_name(std::path::Path::new("/tmp/reclaim"), &version).is_none());
        #[cfg(windows)]
        assert!(
            installer_name(std::path::Path::new("C:/portable/reclaim.exe"), &version).is_some()
        );
        #[cfg(target_os = "linux")]
        assert_eq!(
            installer_name(std::path::Path::new("/usr/bin/reclaim"), &version).as_deref(),
            Some("reclaim_1.2.3_amd64.deb")
        );
        #[cfg(target_os = "macos")]
        assert!(
            installer_name(
                std::path::Path::new("/Applications/Reclaim.app/Contents/MacOS/reclaim"),
                &version
            )
            .unwrap()
            .ends_with(".pkg")
        );
    }
    #[test]
    fn download_rejects_corruption_truncation_and_oversize() {
        let mut r = parse_release(&release(), "test").unwrap().unwrap();
        r.size = 3;
        r.digest = format!("{:x}", Sha256::digest(b"abc"));
        assert!(verify_download(&b"abc"[..], Vec::new(), &r).is_ok());
        for bad in [b"abd".as_slice(), b"ab".as_slice(), b"abcd".as_slice()] {
            assert!(verify_download(bad, Vec::new(), &r).is_err());
        }
    }
    #[test]
    fn rejects_unbounded_public_release_metadata() {
        let mut r = parse_release(&release(), "test").unwrap().unwrap();
        for size in [0, MAX_BINARY + 1, u64::MAX] {
            r.size = size;
            assert!(verify_download(&b"abc"[..], Vec::new(), &r).is_err());
        }
    }

    #[test]
    fn rejects_download_paths_outside_the_exact_release_asset() {
        for suffix in [
            "../other",
            "test?redirect=other",
            "test#fragment",
            "%2e%2e/test",
        ] {
            let mut r = release();
            r["assets"][0]["browser_download_url"] =
                format!("https://github.com/{REPO}/releases/download/v99.0.0/{suffix}").into();
            assert!(parse_release(&r, "test").is_err());
        }
    }

    #[test]
    fn validates_release_contract() {
        assert_eq!(
            parse_release(&release(), "test").unwrap().unwrap().notes,
            "What's new"
        );
        assert!(parse_release(&release(), "other").is_err());
        let mut r = release();
        r["assets"][0]["digest"] = Value::Null;
        assert!(parse_release(&r, "test").is_err());
        r = release();
        r["assets"][0]["browser_download_url"] = "https://evil.example/app".into();
        assert!(parse_release(&r, "test").is_err());
        r = release();
        r["prerelease"] = true.into();
        assert!(parse_release(&r, "test").unwrap().is_none());
        r = release();
        r["tag_name"] = format!("v{}", env!("CARGO_PKG_VERSION")).into();
        assert!(parse_release(&r, "test").unwrap().is_none());
        r = release();
        r["assets"][0]["size"] = (MAX_BINARY + 1).into();
        assert!(parse_release(&r, "test").is_err());
    }
}
