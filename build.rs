extern crate cbindgen;

fn main() {
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
}