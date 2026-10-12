//! Gives the shared library an ELF soname, so a host can tell this library apart from another
//! copy of it already mapped in its process: the dynamic linker identifies a library by its
//! soname, and without one only the file name stands in.

fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "linux" {
        println!("cargo:rustc-cdylib-link-arg=-Wl,-soname,libliteparse_c.so");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
