//! Repository tooling. Kept separate so packaging never compiles the desktop UI.
mod fixtures;
mod package;
mod seeds;

use sha2::{Digest, Sha256};
use std::{
    error::Error,
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
};

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
const IDENTIFIER: &str = "io.github.reclaim.Reclaim";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("tools directory has a parent")
        .to_owned()
}
fn version() -> Result<String> {
    let manifest = toml::Value::Table(fs::read_to_string(root().join("Cargo.toml"))?.parse()?);
    Ok(manifest["package"]["version"]
        .as_str()
        .ok_or("missing package version")?
        .to_owned())
}
fn run(command: &mut Command) -> Result {
    let status = command
        .status()
        .map_err(|e| format!("Cannot run {command:?}: {e}"))?;
    if !status.success() {
        return Err(format!("{command:?} failed: {status}").into());
    }
    Ok(())
}
fn copy(source: impl AsRef<Path>, target: impl AsRef<Path>) -> Result {
    let target = target.as_ref();
    fs::create_dir_all(target.parent().ok_or("copy target has no parent")?)?;
    fs::copy(source, target)?;
    Ok(())
}
fn files(directory: &Path) -> Result<Vec<PathBuf>> {
    let mut result = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            result.extend(files(&entry.path())?);
        } else if kind.is_file() {
            result.push(entry.path());
        }
    }
    result.sort();
    Ok(result)
}
fn checksums(directory: &Path) -> Result {
    for path in files(directory)? {
        if matches!(
            path.extension().and_then(|s| s.to_str()),
            Some("sha256" | "minisig")
        ) {
            continue;
        }
        let mut file = fs::File::open(&path)?;
        let mut digest = Sha256::new();
        let mut buffer = [0; 65536];
        loop {
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            digest.update(&buffer[..count]);
        }
        let name = path
            .file_name()
            .ok_or("download has no name")?
            .to_string_lossy();
        fs::write(
            path.with_file_name(format!("{name}.sha256")),
            format!("{:x}  {name}\n", digest.finalize()),
        )?;
    }
    Ok(())
}
fn bundle(binary: &Path, output: &Path, asset: &str) -> Result {
    // Only a leaf filename can name a release asset.
    if Path::new(asset).components().count() != 1
        || asset.contains(['/', '\\'])
        || matches!(asset, "." | "..")
    {
        return Err("asset must be a leaf filename".into());
    }
    fs::create_dir_all(output)?;
    let executable = output.join(asset);
    copy(binary, &executable)?;
    let mut archive = zip::ZipWriter::new(fs::File::create(output.join(format!("{asset}.zip")))?);
    for (source, name, mode) in [
        (executable, asset, 0o755),
        (root().join("LICENSE"), "LICENSE", 0o644),
        (
            root().join("assets/fonts/DejaVu-LICENSE.txt"),
            "DejaVu-LICENSE.txt",
            0o644,
        ),
    ] {
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(mode);
        archive.start_file(name, options)?;
        std::io::copy(&mut fs::File::open(source)?, &mut archive)?;
    }
    archive.finish()?;
    Ok(())
}
fn main() -> Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("validate-tag") if args.len() == 2 => {
            if args[1] != format!("refs/tags/v{}", version()?) { return Err("select the tag matching Cargo.toml's package version".into()); }
            Ok(())
        }
        Some("bundle") if args.len() == 4 => bundle(Path::new(&args[1]), Path::new(&args[2]), &args[3]),
        Some("checksums") if args.len() == 2 => checksums(Path::new(&args[1])),
        Some("package") => package::main(&args[1..]),
        Some("fixtures") if args.len() <= 2 => fixtures::generate(&args.get(1).map(PathBuf::from).unwrap_or_else(|| root().join("tests/fixtures/formats"))),
        Some("seed-corpus") if args.len() <= 2 => seeds::generate(&args.get(1).map(PathBuf::from).unwrap_or_else(root)),
        _ => Err("Usage: cargo xtask <validate-tag REF | bundle BINARY OUTPUT ASSET | checksums DIR | package PLATFORM --binary PATH [--output DIR] [--arch x86_64|aarch64] | fixtures [DIR] | seed-corpus [REPO]>".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn release_zip_preserves_binary_permissions_and_checksum_contents() -> Result {
        let temporary = tempfile::tempdir()?;
        let binary = temporary.path().join("input");
        fs::write(&binary, b"release executable")?;
        let output = temporary.path().join("output");
        bundle(&binary, &output, "reclaim-linux-x86_64")?;
        let mut archive =
            zip::ZipArchive::new(fs::File::open(output.join("reclaim-linux-x86_64.zip"))?)?;
        assert_eq!(archive.len(), 3);
        let mut entry = archive.by_name("reclaim-linux-x86_64")?;
        assert_eq!(entry.unix_mode().map(|mode| mode & 0o777), Some(0o755));
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        assert_eq!(bytes, b"release executable");
        checksums(&output)?;
        let checksum = fs::read_to_string(output.join("reclaim-linux-x86_64.sha256"))?;
        assert_eq!(
            checksum,
            format!("{:x}  reclaim-linux-x86_64\n", Sha256::digest(&bytes))
        );
        assert!(!output.join("reclaim-linux-x86_64.sha256.sha256").exists());
        assert!(bundle(&binary, &output, "../escape").is_err());
        Ok(())
    }
}
