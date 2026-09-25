use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=../lzma_rust_api.h");

    // Handle prefix
    println!("cargo:rustc-env=LZMA_SYS_PREFIX=");

    // Version
    println!("cargo:rustc-env=LZMA_VERSION=5.8.3");

    let bindings = bindgen::Builder::default()
        .header("../lzma_rust_api.h")
        .with_codegen_config(bindgen::CodegenConfig::TYPES | bindgen::CodegenConfig::VARS)
        .generate_comments(false)
        .default_macro_constant_type(bindgen::MacroTypeVariation::Unsigned)
        .prepend_enum_name(false)
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings.write_to_file(out_path.join("lzma_types_gen.rs")).expect("Couldn't write bindings!");
}
