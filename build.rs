fn main() {
    println!("cargo:rerun-if-changed=assets/icons/reclaim.ico");
    #[cfg(windows)]
    winresource::WindowsResource::new()
        .set_icon("assets/icons/reclaim.ico")
        .set("ProductName", "Reclaim")
        .set("FileDescription", "Reclaim — File Recovery")
        .compile()
        .expect("compile Windows application icon and version metadata");
}
