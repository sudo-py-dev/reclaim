fn main() {
    let config = slint_build::CompilerConfiguration::new()
        .with_debug_info(std::env::var("PROFILE").as_deref() == Ok("debug"));
    slint_build::compile_with_config("ui/app.slint", config).expect("compile Slint interface");
    println!("cargo:rerun-if-changed=assets/icons/reclaim.ico");
    #[cfg(windows)]
    winresource::WindowsResource::new()
        .set_icon("assets/icons/reclaim.ico")
        .set("ProductName", "Reclaim")
        .set("FileDescription", "Reclaim — File Recovery")
        .compile()
        .expect("compile Windows application icon and version metadata");
}
