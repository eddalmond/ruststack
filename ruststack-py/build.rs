fn main() {
    // Don't link libpython - use extension-module feature instead
    println!("cargo:rustc-cfg=pyo3_extension_module");
}
