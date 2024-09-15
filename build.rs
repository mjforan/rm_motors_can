extern crate cbindgen;
use std::fs;
use std::process::{Command, Stdio};
use std::env;

fn main() {
// TODO clean up, better comments

  println!("cargo:rerun-if-changed=build.rs");

  // Don't do any custom build steps if this is being run by our manual invocation of `cargo expand` (avoid infinite recursion)
  if env::var("CARGO_EXPAND").is_ok() {
      return;
  }

  // Run `cargo expand` to expand macros into the output file
  // TODO add --release if necessary
  let output = Command::new("cargo")
      .arg("expand")
      .env("CARGO_EXPAND", "true")
      .arg("--lib")
      .arg("--ugly")
      .stdout(Stdio::piped())
      .output()
      .expect("Failed to run `cargo expand`");
  println!("Writing expanded library file");
  fs::rename("src/lib.rs", "src/lib").expect("Unable to move current lib.rs");
  // I don't like touching source files but this is the only way I could find to make this work.
  fs::write("src/lib.rs", String::from_utf8(output.stdout).unwrap()).expect("Unable to write expanded library file");

  // Use cbindgen to create a C header
  println!("Generating C/C++ header");
  let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
  let output_path = std::path::PathBuf::from(crate_dir.clone()).join("include").join("gm6020_can.h");
  cbindgen::generate(crate_dir).expect("Unable to generate bindings").write_to_file(output_path);

  // TODO bug in cc where it is unable to link to system libraries for an example in a lib crate  https://github.com/rust-lang/cc-rs/issues/1206
  // Compile the C++ example
  // There is no good way to do this conditionally i.e. only when `cargo build --examples`


  println!("cargo:rerun-if-changed=examples/gm6020_can_test.cpp");
  cc::Build::new()
  .cpp(true)
  .file("examples/gm6020_can_test.cpp")
  .cpp_link_stdlib("stdc++")
  .compile("gm6020_can_test_cpp");

  fs::remove_file("src/lib.rs").expect("Unable to delete expanded lib.rs");
  fs::rename("src/lib", "src/lib.rs").expect("Unable to restore lib.rs");
}