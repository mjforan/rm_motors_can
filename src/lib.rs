use gm6020_can::*;

use std::ffi::CStr;
use std::os::raw::c_char;
use std::ptr::null;
use std::sync::Arc;

// TODO handle CAN no buffer left
// TODO try to reduce panics and handle errors more gracefully

/*
**  interface: SocketCAN interface name e.g. "can0"
**  returns: pointer to Gm6020Can struct, to be passed to other functions in this library
*/
#[no_mangle]
pub extern "C" fn init(interface: *const c_char) -> *mut Gm6020Can {
    let inter: &str;
    if interface.is_null() {
        println!("Invalid c-string received for interface name (null pointer)");
        return null::<Gm6020Can>() as *mut Gm6020Can;
    }
    else {
        unsafe {
            let r = CStr::from_ptr(interface).to_str();
            if r.is_err() {
                eprintln!("Invalid c-string received for interface name");
                return null::<Gm6020Can>() as *mut Gm6020Can;
            }
            inter = r.unwrap();
        }
    }
    gm6020_can::init(inter).map_or_else(|e| {eprintln!("{}", e); null::<Gm6020Can>() as *const Gm6020Can}, |v| Arc::into_raw(v)) as *mut Gm6020Can
}

macro_rules! generate_wrapper {
    ($func_name:ident, ($($param_name:ident: $param_type:ty),*), $return_type:ty) => {
        #[no_mangle]
        pub extern "C" fn $func_name(gm6020_can: *mut Gm6020Can, $($param_name: $param_type),*) -> $return_type {
            if gm6020_can.is_null(){
                println!("Invalid handle (null pointer)");
                return -1 as $return_type;
            }

            let gm6020_can: Arc<Gm6020Can> = unsafe { Arc::from_raw(gm6020_can as *const Gm6020Can) }; // reconstitute the Arc temporarily to clone it
            let gm6020_can_ref2 = Arc::clone(&gm6020_can);
            std::mem::forget(gm6020_can); // "forget" the Arc to avoid dropping it (since C++ still needs to reuse it)
            gm6020_can::$func_name(gm6020_can_ref2, $($param_name),*).map_or_else(|e| {eprintln!("{}", e); -1 as $return_type}, |v| v as $return_type)
        }
    };
}

generate_wrapper!(cleanup,   (period_ms: u64), i32);
generate_wrapper!(run_once,  (), i32);
generate_wrapper!(set_cmd,   (id: u8, mode: CmdMode, cmd: f64), i32);
generate_wrapper!(get_state, (id: u8, field: FbField), f64);

#[link(name = "gm6020_can_test_cpp")]
extern { fn gm6020_can_test_cpp() -> i32; }
// TODO this is only here due to a bug in the cc crate preventing c++ in examples: https://github.com/rust-lang/cc-rs/issues/1206
pub unsafe fn cpp_example(){
    std::process::exit(gm6020_can_test_cpp());
}