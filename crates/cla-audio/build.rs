//! Compiles the vendored ggwave (vendor/ggwave/VENDORED.md) as a static C++ library.

fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++11")
        // The shim includes vendor/ggwave/src/ggwave.cpp (one translation unit).
        .file("shim/mem_ggwave.cpp")
        .include("vendor/ggwave/include")
        .include("vendor/ggwave/src")
        .define("NDEBUG", None)
        .flag_if_supported("-fno-exceptions")
        .flag_if_supported("-fno-rtti")
        .warnings(false);
    if target.contains("android") {
        // The APK carries no shared C++ runtime: link it statically into libmem_ffi.so.
        build.cpp_link_stdlib(Some("c++_static"));
        println!("cargo:rustc-link-lib=c++abi");
    }
    build.compile("ggwave");
    println!("cargo:rerun-if-changed=vendor/ggwave");
    println!("cargo:rerun-if-changed=shim");
}
