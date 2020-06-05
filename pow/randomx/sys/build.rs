use std::env;
use std::path::PathBuf;

fn main() {
    let target = env::var("TARGET").unwrap();

    let mut config = cmake::Config::new("randomx");
	config.define("ARCH", "native");

	if target.contains("windows") {
		config.build_target("ALL_BUILD")
	} else {
		config.build_target("all")
	};

	let dst = config.build();

    if target.contains("pc-windows-msvc") {
        println!("cargo:rustc-link-search=native={}/build/Release", dst.display());
    } else {
        println!("cargo:rustc-link-search=native={}/build", dst.display());
    }
    println!("cargo:rustc-link-lib=static=randomx");
    if target.contains("apple-darwin") {
        println!("cargo:rustc-link-lib=dylib=c++");
    } else {
        println!("cargo:rustc-link-lib=dylib=stdc++");
    }

    // The bindgen::Builder is the main entry point
    // to bindgen, and lets you build up options for
    // the resulting bindings.
    let bindings = bindgen::Builder::default()
        // The input header we would like to generate
        // bindings for.
        .header("randomx/src/randomx.h")
        // Workaround for https://github.com/servo/rust-bindgen/issues/550
        .blacklist_type("max_align_t")
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
