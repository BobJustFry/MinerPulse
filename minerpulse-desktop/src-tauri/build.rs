fn main() {
    let version_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../VERSION.json");
    let version_json = std::fs::read_to_string(version_path).expect("VERSION.json missing");
    let escaped = version_json.replace('\\', "\\\\").replace('"', "\\\"");
    println!("cargo:rustc-env=MINERPULSE_VERSION_JSON={}", escaped);
    tauri_build::build()
}
