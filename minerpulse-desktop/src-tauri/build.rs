fn main() {
    let version_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../VERSION.json");
    println!("cargo:rerun-if-changed={}", version_path.display());

    let version_json = std::fs::read_to_string(&version_path).expect("VERSION.json missing");
    let value: serde_json::Value =
        serde_json::from_str(&version_json).expect("VERSION.json must be valid JSON");
    // cargo:rustc-env must be a single line; pretty-printed JSON breaks the directive.
    let compact = value.to_string();
    println!("cargo:rustc-env=MINERPULSE_VERSION_JSON={compact}");

    tauri_build::build()
}
