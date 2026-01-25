use std::env;
use std::path::PathBuf;

fn main() {
    if env::var_os("CARGO_FEATURE_KLU").is_none() {
        return;
    }

    let include_dir = env::var_os("KLU_INCLUDE_DIR")
        .map(PathBuf::from)
        .or_else(|| env::var_os("SUITESPARSE_DIR").map(PathBuf::from).map(|p| p.join("include")));
    let lib_dir = env::var_os("KLU_LIB_DIR")
        .map(PathBuf::from)
        .or_else(|| env::var_os("SUITESPARSE_DIR").map(PathBuf::from).map(|p| p.join("lib")));

    if let Some(dir) = lib_dir.as_ref() {
        println!("cargo:rustc-link-search=native={}", dir.display());
    } else {
        println!("cargo:warning=KLU_LIB_DIR or SUITESPARSE_DIR not set");
    }

    if include_dir.is_none() {
        println!("cargo:warning=KLU_INCLUDE_DIR or SUITESPARSE_DIR not set");
    }

    println!("cargo:rustc-link-lib=klu");
    println!("cargo:rustc-link-lib=amd");
    println!("cargo:rustc-link-lib=colamd");
    println!("cargo:rustc-link-lib=suitesparseconfig");
}
