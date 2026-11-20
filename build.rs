use std::{
    error::Error,
    fmt::Write as _,
    fs::{self, File},
    io::Read as _,
    path::{Path, PathBuf},
};

/// Upper bound for the locale index; the real file is well under 1 KiB.
const LANGUAGES_LIMIT: u64 = 64 * 1024;

fn main() -> Result<(), Box<dyn Error>> {
    generate_languages()?;
    let config = slint_build::CompilerConfiguration::new()
        .with_debug_info(std::env::var("PROFILE").as_deref() == Ok("debug"));
    slint_build::compile_with_config("ui/app.slint", config)?;
    // audit-allow(CODE-DEBUG): this is Cargo's build-script protocol, not debug output.
    println!("cargo:rerun-if-changed=assets/icons/reclaim.ico");
    #[cfg(windows)]
    winresource::WindowsResource::new()
        .set_icon("assets/icons/reclaim.ico")
        .set("ProductName", "Reclaim")
        .set("FileDescription", "Reclaim - File Recovery")
        .compile()?;
    Ok(())
}

fn generate_languages() -> Result<(), Box<dyn Error>> {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let locales = root.join("locales");
    // audit-allow(CODE-DEBUG): this is Cargo's build-script protocol, not debug output.
    println!("cargo:rerun-if-changed=locales");
    let mut generated = String::from("define_languages! {\n");
    for row in read_limited(&locales.join("languages.txt"), LANGUAGES_LIMIT)?.lines() {
        let (variant, rest) = row.split_once(' ').ok_or("missing locale code")?;
        let (code, name) = rest.split_once(' ').ok_or("missing locale name")?;
        let path = locales.join(format!("{code}.json"));
        if path.is_file() {
            writeln!(generated, "    {variant}, {code:?}, {name:?}, {:?};", path)?;
        }
    }
    generated.push_str("}\n");
    let output = PathBuf::from(std::env::var("OUT_DIR")?).join("languages.rs");
    fs::write(output, generated)?;
    Ok(())
}

fn read_limited(path: &Path, limit: u64) -> Result<String, Box<dyn Error>> {
    let mut text = String::new();
    File::open(path)?.take(limit + 1).read_to_string(&mut text)?;
    if text.len() as u64 > limit {
        return Err(format!("{} exceeds {limit} bytes", path.display()).into());
    }
    Ok(text)
}
