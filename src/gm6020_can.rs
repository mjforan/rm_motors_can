#[no_mangle]
pub extern "C" fn rust_function(x: i32, y:i32) -> i32 {
    x + y
}
