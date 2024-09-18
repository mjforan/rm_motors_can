extern crate cbindgen;
use std::fs;
use std::process::{Command, Stdio};
use std::env;
use std::path::PathBuf;

fn main() {
  println!("cargo:rerun-if-changed=include/gm6020_can.h");
  println!("cargo:rerun-if-changed=include/gm6020_can.hpp");

  // Don't do any custom build steps if this is being run by our manual invocation of `cargo expand` (avoid infinite recursion)
  if env::var("CARGO_EXPAND").is_ok() {
      return;
  }

  // Run `cargo expand` to expand macros into the src/expanded.rs. This is necessary because cbindgen can't see the function definitions otherwise.
  // Actually cbindgen can expand on its own, but without passing the `--lib` flag it will fail when it tries to compile the c++ example, which may not have a header generated yet
  println!("Expanding macros");
  let output = Command::new("cargo")
      .arg("expand")
      .env("CARGO_EXPAND", "true")
      .arg("--lib")
      .arg("--ugly")
      .stdout(Stdio::piped())
      .output()
      .expect("Failed to expand macros. Did you `cargo install cargo-expand`?");
  fs::write("src/expanded.rs", String::from_utf8(output.stdout).unwrap()).expect("Unable to write expanded library file");

  // Use cbindgen to generate C/C++ headers
  println!("Generating C/C++ headers");
  let crate_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
  let config_c: cbindgen::Config = cbindgen::Config::from_file((&crate_dir).join("cbindgen_c.toml")).expect("Failed to load cbindgen configuration for C");
  let config_cpp: cbindgen::Config = cbindgen::Config::from_file((&crate_dir).join("cbindgen_cpp.toml")).expect("Failed to load cbindgen configuration for C++");

  cbindgen::Builder::new()
    .with_src((&crate_dir).join("src/expanded.rs"))
    .with_crate(&crate_dir)
    .with_config(config_c)
    .generate()
    .expect("Failed to generate C header")
    .write_to_file("include/gm6020_can.h");

  cbindgen::Builder::new()
    .with_src((&crate_dir).join("src/expanded.rs"))
    .with_crate(&crate_dir)
    .with_config(config_cpp)
    .generate()
    .expect("Failed to generate C++ header")
    .write_to_file("include/gm6020_can.hpp");


  // TODO bug in cc where it is unable to link to system libraries for an example in a lib crate  https://github.com/rust-lang/cc-rs/issues/1206
  // Compile the C++ example
  // There is no good way to do this conditionally i.e. only when `cargo build --examples`

  println!("cargo:rerun-if-changed=examples/gm6020_can_test.cpp");
  cc::Build::new()
  .cpp(true)
  .file("examples/gm6020_can_test.cpp")
  .cpp_link_stdlib("stdc++")
  .compile("gm6020_can_test_cpp");

  fs::remove_file("src/expanded.rs").expect("Unable to delete expanded lib.rs"); // Remove the temporary file
}