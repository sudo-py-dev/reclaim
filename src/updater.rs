//! Portable executable updates from this project's stable GitHub releases.
//!
//! Every update must carry a Minisign (Ed25519) signature made by the release
//! key whose public half is compiled in below. The signature's trusted comment
//! names the exact file and version, so a validly signed older build cannot be
//! re-published as a newer one. The GitHub SHA-256 digest is checked as well,
//! but on its own it only proves the file matches what the release page lists.
use minisign_verify::{PublicKey, Signature};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    time::Duration,
};

const REPO: &str = "sudo-py-dev/reclaim";
const MAX_BINARY: u64 = 128 * 1024 * 1024;
const USER_AGENT: &str = concat!("Reclaim/", env!("CARGO_PKG_VERSION"));
const MAX_SIGNATURE: u64 = 4096;
#[cfg(not(test))]
const TRUSTED_KEY: &str = include_str!("../packaging/update-signing.pub");
// Unit tests use a separate throwaway key; its secret half is not kept anywhere.
#[cfg(test)]
const TRUSTED_KEY: &str = "untrusted comment: minisign public key: 1F2EE97A0817BA50
RWRQuhcIeukuH8FmXH9gkagLsXG8ykzBa6/5B3NcIDO7etyG2LjuONLq
";

#[derive(Clone, Debug)]
pub struct Release {
    pub version: semver::Version,
    pub asset: String,
    pub notes: String,
    pub url: String,
    pub digest: String,
    pub size: u64,
    /// `<asset>.minisig` on the same release; fetched by [`check`].
    pub signature_url: String,
    pub signature: String,
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

/// Every request identifies the app, as GitHub's API requires.
fn get(url: &str) -> ureq::RequestBuilder<ureq::typestate::WithoutBody> {
    agent().get(url).header("User-Agent", USER_AGENT)
}

fn release_url(tag: &str, asset: &str) -> String {
    format!("https://github.com/{REPO}/releases/download/{tag}/{asset}")
}

fn check_error(error: ureq::Error) -> String {
    log::warn!("Could not check GitHub releases: {error}");
    match error {
        ureq::Error::Io(_)
        | ureq::Error::HostNotFound
        | ureq::Error::ConnectionFailed
        | ureq::Error::Timeout(_) => {
            "Could not connect to the update server. Check your internet connection and try again."
        }
        _ => "Could not check for updates. Please try again later.",
    }
    .into()
}

pub fn check() -> Result<Option<Release>, String> {
    let mut response = get(&format!(
        "https://api.github.com/repos/{REPO}/releases/latest"
    ))
    .header("Accept", "application/vnd.github+json")
    .call()
    .map_err(check_error)?;
    let json = response
        .body_mut()
        .with_config()
        .limit(2 * 1024 * 1024)
        .read_to_string()
        .map_err(check_error)?;
    let json: Value = serde_json::from_str(&json).map_err(|e| e.to_string())?;
    let version = semver::Version::parse(
        json["tag_name"]
            .as_str()
            .unwrap_or("")
            .trim_start_matches('v'),
    )
    .map_err(|e| e.to_string())?;
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let asset = installer_name(&executable, &version).unwrap_or_else(asset_name);
    let Some(mut release) = parse_release(&json, &asset)? else {
        return Ok(None);
    };
    release.signature = get(&release.signature_url)
        .call()
        .map_err(check_error)?
        .body_mut()
        .with_config()
        .limit(MAX_SIGNATURE)
        .read_to_string()
        .map_err(check_error)?;
    signed_by_release_key(&release)?;
    Ok(Some(release))
}

/// The trusted comment the release workflow signs into every `.minisig`.
fn signed_comment(release: &Release) -> String {
    format!(
        "reclaim-update file:{} version:{}",
        release.asset, release.version
    )
}

/// Decode the signature and check it was made by the release key for exactly
/// this file and version. The file contents are verified while downloading.
fn signed_by_release_key(release: &Release) -> Result<(PublicKey, Signature), String> {
    const UNSIGNED: &str = "This update is not signed by the Reclaim release key";
    let key = PublicKey::decode(TRUSTED_KEY).map_err(|_| "Invalid built-in release key")?;
    let signature = Signature::decode(&release.signature).map_err(|_| UNSIGNED)?;
    // Binds file name and version; the comment itself is covered by the signature.
    if signature.trusted_comment() != signed_comment(release) {
        return Err(UNSIGNED.into());
    }
    // Rejects other keys and legacy (non-prehashed) signatures before any download.
    key.verify_stream(&signature).map_err(|_| UNSIGNED)?;
    Ok((key, signature))
}

fn parse_release(json: &Value, asset_name: &str) -> Result<Option<Release>, String> {
    parse_release_after(
        json,
        asset_name,
        &semver::Version::parse(env!("CARGO_PKG_VERSION")).unwrap(),
    )
}

fn parse_release_after(
    json: &Value,
    asset_name: &str,
    current: &semver::Version,
) -> Result<Option<Release>, String> {
    if json["draft"].as_bool() != Some(false) || json["prerelease"].as_bool() != Some(false) {
        return Ok(None);
    }
    let tag = json["tag_name"].as_str().ok_or("Missing release version")?;
    let version = semver::Version::parse(tag.trim_start_matches('v')).map_err(|e| e.to_string())?;
    if !version.pre.is_empty() || version <= *current {
        return Ok(None);
    }
    let asset = json["assets"]
        .as_array()
        .ok_or("Missing release assets")?
        .iter()
        .find(|a| a["name"].as_str() == Some(asset_name))
        .ok_or("No update for this platform yet")?;
    let signature_name = format!("{asset_name}.minisig");
    let signature_url = json["assets"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|a| a["name"].as_str() == Some(&signature_name))
        .and_then(|a| a["browser_download_url"].as_str())
        .filter(|url| *url == release_url(tag, &signature_name))
        .ok_or("This update is not signed by the Reclaim release key")?
        .to_owned();
    let url = asset["browser_download_url"]
        .as_str()
        .ok_or("Missing download URL")?;
    if url != release_url(tag, asset_name) {
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
    let release = Release {
        version,
        asset: asset_name.into(),
        notes: json["body"].as_str().unwrap_or("No release notes.").into(),
        url: url.into(),
        digest,
        size,
        signature_url,
        signature: String::new(),
    };
    // Also rejects tags that are not exactly `vX.Y.Z` or `X.Y.Z` for the parsed version.
    validate_url(&release)?;
    Ok(Some(release))
}

/// Installed packages must be updated as packages to preserve receipts and uninstall metadata.
pub fn installer_download(release: &Release) -> Option<String> {
    is_installer(&release.asset).then(|| release.url.clone())
}

fn is_installer(name: &str) -> bool {
    name.ends_with("-setup.exe") || name.ends_with(".deb") || name.ends_with(".pkg")
}

/// Download the actual package asset, verify it, then atomically publish the
/// selected destination. Failed downloads leave no partial installer behind.
pub fn download_installer(
    release: &Release,
    destination: &std::path::Path,
    progress: impl Fn(u64, u64),
) -> Result<(), String> {
    expect_installer(release)?;
    let mut response = get(&release.url)
        .call()
        .map_err(|e| format!("Could not download update: {e}"))?;
    publish_download(
        response.body_mut().as_reader(),
        release,
        destination,
        progress,
    )
}

fn publish_download(
    reader: impl Read,
    release: &Release,
    destination: &std::path::Path,
    progress: impl Fn(u64, u64),
) -> Result<(), String> {
    let parent = destination
        .parent()
        .ok_or("Download destination has no parent folder")?;
    let mut pending = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    verify_download_progress(reader, &mut pending, release, progress)?;
    pending.as_file().sync_all().map_err(|e| e.to_string())?;
    pending
        .persist(destination)
        .map_err(|e| e.error.to_string())?;
    Ok(())
}

fn validate_url(release: &Release) -> Result<(), String> {
    if release.asset.is_empty() || release.asset.contains(['/', '\\', '?', '#', '%']) {
        return Err("Unexpected release asset name".into());
    }
    let version = release.version.to_string();
    if [format!("v{version}"), version]
        .iter()
        .any(|tag| release.url == release_url(tag, &release.asset))
    {
        Ok(())
    } else {
        Err("Unexpected release download URL".into())
    }
}

/// The release must be the package format this installation was installed with.
fn expect_installer(release: &Release) -> Result<(), String> {
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    if installer_name(&executable, &release.version).as_deref() != Some(&release.asset) {
        return Err("Unexpected installer for this installation".into());
    }
    validate_url(release)
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
    install_with_progress(release, |_, _| {})
}

pub fn install_with_progress(release: &Release, progress: impl Fn(u64, u64)) -> Result<(), String> {
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    if installer_name(&executable, &release.version).is_some()
        || installer_download(release).is_some()
    {
        return Err("This installation must be updated using the matching installer.".into());
    }
    if release.asset != asset_name() {
        return Err("Unexpected executable update asset".into());
    }
    validate_url(release)?;
    if cfg!(windows) {
        return Err("This installation must be updated using the matching installer.".into());
    }
    // Private temporary file, bounded streaming download, verify before replacing anything.
    let mut file = tempfile::NamedTempFile::new().map_err(|e| e.to_string())?;
    let mut response = get(&release.url).call().map_err(|e| e.to_string())?;
    verify_download_progress(
        response.body_mut().as_reader(),
        &mut file,
        release,
        progress,
    )?;
    file.as_file().sync_all().map_err(|e| e.to_string())?;
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

/// Recheck a staged package immediately before handing it to the native installer.
pub fn verify_installer_file(release: &Release, path: &std::path::Path) -> Result<(), String> {
    expect_installer(release)?;
    verify_download_progress(
        std::fs::File::open(path).map_err(|e| e.to_string())?,
        std::io::sink(),
        release,
        |_, _| {},
    )
}

#[cfg(test)]
fn verify_download(
    reader: impl Read,
    mut file: impl Write,
    release: &Release,
) -> Result<(), String> {
    verify_download_progress(reader, &mut file, release, |_, _| {})
}

fn verify_download_progress(
    reader: impl Read,
    mut file: impl Write,
    release: &Release,
    progress: impl Fn(u64, u64),
) -> Result<(), String> {
    // Release is public: enforce bounds here as well as when parsing the API.
    if release.size == 0
        || release.size > MAX_BINARY
        || release.digest.len() != 64
        || !release.digest.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("Invalid update metadata".into());
    }
    let (key, signature) = signed_by_release_key(release)?;
    let mut verifier = key
        .verify_stream(&signature)
        .map_err(|_| "This update is not signed by the Reclaim release key")?;
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
        verifier.update(&buffer[..count]);
        progress(size, release.size);
        file.write_all(&buffer[..count])
            .map_err(|e| e.to_string())?;
    }
    if size != release.size || format!("{:x}", hash.finalize()) != release.digest {
        return Err("Update checksum verification failed".into());
    }
    verifier
        .finalize()
        .map_err(|_| "Update signature verification failed".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    // Made with the throwaway test key over b"abc"; only the trusted comment differs.
    const SIGNED_ABC: &str = "untrusted comment: signature from rsign secret key
RURQuhcIeukuH6zpT7RnewVaNNt+tOUzOJG59dA3rWm2wZh6GMIkx0mK30xQg2ddxgiEFXYrnLEfoInpB2FaDzQeD7w5Wj/upQc=
trusted comment: reclaim-update file:test version:99.0.0
ZAvWFv9RyzxWyPwjTB8k83Hxu8v41hr+rwHBadGYetYRL2RTq/Y5178a4Xicjg4oJaymUJEXszXuT/wI8YoODA==
";
    const SIGNED_ABC_AS_1_0_0: &str = "untrusted comment: signature from rsign secret key
RURQuhcIeukuHxs8ZziVxPftaLiWBhPMNU541wueN/qa4yVO8ThvxWsntUybwfk+t6U3bqgrGV4/AlTsFmuvqgqO9b1g0/QNCwk=
trusted comment: reclaim-update file:test version:1.0.0
MbpIWMky15o9MmvgFruTTeC1TX8xqIARe0bHCg69++aqfnx8HJDHPdy6DG0miEizEhAabxrfS0lYPwBTAH7GCA==
";
    fn signature_asset(name: &str) -> Value {
        serde_json::json!({"name": format!("{name}.minisig"), "size": 300,
            "browser_download_url": format!("https://github.com/{REPO}/releases/download/v99.0.0/{name}.minisig")})
    }
    fn release() -> Value {
        serde_json::json!({"draft":false,"prerelease":false,"tag_name":"v99.0.0",
            "body":"What's new", "assets":[{"name":"test", "size":123,
            "digest":format!("sha256:{}", "a".repeat(64)),
            "browser_download_url":"https://github.com/sudo-py-dev/reclaim/releases/download/v99.0.0/test"},
            signature_asset("test")]})
    }
    /// A parsed release for the 3-byte payload b"abc", signed by the test key.
    fn signed_abc() -> Release {
        let mut r = parse_release(&release(), "test").unwrap().unwrap();
        r.size = 3;
        r.digest = format!("{:x}", Sha256::digest(b"abc"));
        r.signature = SIGNED_ABC.into();
        r
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
        let r = signed_abc();
        assert!(verify_download(&b"abc"[..], Vec::new(), &r).is_ok());
        for bad in [b"abd".as_slice(), b"ab".as_slice(), b"abcd".as_slice()] {
            assert!(verify_download(bad, Vec::new(), &r).is_err());
        }
    }
    #[test]
    fn rejects_unbounded_public_release_metadata() {
        let mut r = signed_abc();
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
    #[ignore = "downloads a real installer from GitHub over HTTPS"]
    fn live_https_installer_download() {
        let json: Value = serde_json::from_str(
            &agent()
                .get(&format!(
                    "https://api.github.com/repos/{REPO}/releases/latest"
                ))
                .header("User-Agent", "Reclaim-update-test")
                .call()
                .unwrap()
                .body_mut()
                .with_config()
                .limit(2 * 1024 * 1024)
                .read_to_string()
                .unwrap(),
        )
        .unwrap();
        let version = json["tag_name"].as_str().unwrap().trim_start_matches('v');
        let name = if cfg!(windows) {
            "reclaim-windows-x86_64-setup.exe".into()
        } else if cfg!(target_os = "macos") {
            format!("reclaim-macos-{}.pkg", std::env::consts::ARCH)
        } else {
            format!("reclaim_{version}_amd64.deb")
        };
        let selected = parse_release_after(&json, &name, &semver::Version::new(0, 0, 0))
            .unwrap()
            .unwrap();
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join(&name);
        let mut response = agent().get(&selected.url).call().unwrap();
        publish_download(
            response.body_mut().as_reader(),
            &selected,
            &destination,
            |_, _| {},
        )
        .unwrap();
        assert_eq!(std::fs::metadata(destination).unwrap().len(), selected.size);
    }

    #[test]
    fn packages_use_their_own_download_size_and_checksum() {
        let mut json = release();
        let mut package = json["assets"][0].clone();
        package["name"] = "reclaim_99.0.0_amd64.deb".into();
        package["browser_download_url"] =
            format!("https://github.com/{REPO}/releases/download/v99.0.0/reclaim_99.0.0_amd64.deb")
                .into();
        package["digest"] = format!("sha256:{}", "b".repeat(64)).into();
        package["size"] = 456.into();
        json["assets"].as_array_mut().unwrap().push(package);
        json["assets"]
            .as_array_mut()
            .unwrap()
            .push(signature_asset("reclaim_99.0.0_amd64.deb"));
        let selected = parse_release(&json, "reclaim_99.0.0_amd64.deb")
            .unwrap()
            .unwrap();
        assert_eq!(selected.size, 456);
        assert_eq!(selected.digest, "b".repeat(64));
        assert_eq!(
            installer_download(&selected).as_deref(),
            Some(selected.url.as_str())
        );
    }

    #[test]
    fn failed_downloads_never_publish_or_replace_an_installer() {
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("update.deb");
        let selected = signed_abc();
        assert!(publish_download(&b"bad"[..], &selected, &destination, |_, _| {}).is_err());
        assert!(!destination.exists());
        std::fs::write(&destination, b"existing").unwrap();
        assert!(publish_download(&b"ab"[..], &selected, &destination, |_, _| {}).is_err());
        assert_eq!(std::fs::read(&destination).unwrap(), b"existing");
        let progress = std::cell::Cell::new(0);
        publish_download(&b"abc"[..], &selected, &destination, |done, total| {
            assert_eq!(total, 3);
            progress.set(done);
        })
        .unwrap();
        assert_eq!(progress.get(), 3);
        assert_eq!(std::fs::read(&destination).unwrap(), b"abc");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn only_updates_signed_by_the_release_key_for_this_version_install() {
        let good = signed_abc();
        assert!(verify_download(&b"abc"[..], Vec::new(), &good).is_ok());
        // Unsigned, garbage, or a valid signature for a different version/file.
        for signature in ["", "not a signature", SIGNED_ABC_AS_1_0_0] {
            let mut r = good.clone();
            r.signature = signature.into();
            assert!(
                verify_download(&b"abc"[..], Vec::new(), &r).is_err(),
                "{signature}"
            );
        }
        let mut renamed = good.clone();
        renamed.asset = "other".into();
        assert!(signed_by_release_key(&renamed).is_err());
        // Contents that match a forged digest but not the signature.
        let mut forged = good.clone();
        forged.digest = format!("{:x}", Sha256::digest(b"abd"));
        assert!(verify_download(&b"abd"[..], Vec::new(), &forged).is_err());
        // A release without a matching .minisig asset is not offered at all.
        let mut json = release();
        json["assets"].as_array_mut().unwrap().pop();
        assert!(parse_release(&json, "test").is_err());
        json["assets"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"name": "test.minisig",
                "browser_download_url": "https://evil.example/test.minisig"}));
        assert!(parse_release(&json, "test").is_err());
    }

    #[test]
    fn built_in_release_key_is_valid() {
        PublicKey::decode(include_str!("../packaging/update-signing.pub")).unwrap();
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

#[cfg(test)]
mod connection_message_tests {
    use super::check_error;

    #[test]
    fn connection_errors_give_actionable_messages_without_system_details() {
        let expected =
            "Could not connect to the update server. Check your internet connection and try again.";
        for error in [
            ureq::Error::Io(std::io::Error::other(
                "failed to lookup address information: Temporary failure in name resolution",
            )),
            ureq::Error::HostNotFound,
            ureq::Error::ConnectionFailed,
        ] {
            assert_eq!(check_error(error), expected);
        }
    }

    #[test]
    fn server_errors_do_not_claim_the_user_is_offline() {
        assert_eq!(
            check_error(ureq::Error::StatusCode(503)),
            "Could not check for updates. Please try again later."
        );
    }
}
