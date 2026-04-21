fn main() {
    println!("cargo:rustc-link-search=none");
    println!("cargo:rustc-link-lib=python313");
    tauri_build::build()
}
