fn main() {
    glib_build_tools::compile_resources(
        &["."],
        "gresource.xml",
        "resources.gresource",
    );

    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    std::fs::write(out.join("build_timestamp"), now.to_string()).unwrap();
}
