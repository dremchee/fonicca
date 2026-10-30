fn main() {
    let root = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let catalog = root.join("../model-packs/catalog.json");
    println!("cargo:rerun-if-changed={}", catalog.display());
    let text = std::fs::read_to_string(catalog).expect("model catalog is part of the source checkout");
    let text = text.replace("\r\n", "\n");
    let fingerprint = text.bytes().fold(14695981039346656037u64, |hash, byte|
        (hash ^ u64::from(byte)).wrapping_mul(1099511628211));
    let marker = format!("// Catalog FNV-1a64: {fingerprint:016x}");
    for path in ["src/stems/catalog.generated.rs", "../src/model-catalog.generated.ts"] {
        let path = root.join(path);
        println!("cargo:rerun-if-changed={}", path.display());
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(text.lines().any(|line| line == marker),
            "{} is stale; run pnpm models:generate", path.display());
    }
    tauri_build::build()
}
