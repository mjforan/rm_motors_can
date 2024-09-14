  fn main(){
    
  // Use cbindgen to create a C header
  println!("Generating C/C++ header");
  let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
  let output_path = std::path::PathBuf::from(crate_dir.clone()).join("include").join("gm6020_can.h");
  cbindgen::generate(crate_dir).expect("Unable to generate bindings").write_to_file(output_path);

  }