use std::path::Path;

fn main() {
    let out_dir = std::env::var("OUT_DIR").unwrap();

    println!("cargo:rerun-if-changed=src/");
    println!("cargo:rerun-if-changed=Cargo.toml");

    let wasm_path = Path::new(&out_dir)
        .ancestors()
        .nth(5)
        .unwrap()
        .join("wasm32-unknown-unknown")
        .join("release")
        .join("alkanes_stake.wasm");

    if wasm_path.exists() {
        println!("cargo:warning=WASM file ready: {:?}", wasm_path);
    }
}