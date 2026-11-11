use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
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
