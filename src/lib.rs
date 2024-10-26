use rm_motors_can::*;

use std::ffi::CStr;
use std::os::raw::c_char;
use std::ptr::null;
use std::sync::Arc;

/*
**  interface: SocketCAN interface name e.g. "can0"
**  returns: pointer to RmMotorsCan struct, to be passed to other functions in this library
*/
#[no_mangle]
pub extern "C" fn init_bus(interface: *const c_char) -> *mut RmMotorsCan {
    let inter: &str;
    if interface.is_null() {
        println!("Invalid c-string received for interface name (null pointer)");
        return null::<RmMotorsCan>() as *mut RmMotorsCan;
    }
    else {
        unsafe {
            let r = CStr::from_ptr(interface).to_str();
            if r.is_err() {
                eprintln!("Invalid c-string received for interface name");
                return null::<RmMotorsCan>() as *mut RmMotorsCan;
            }
            inter = r.unwrap();
        }
    }
    rm_motors_can::init_bus(inter).map_or_else(|e| {eprintln!("{}", e); null::<RmMotorsCan>() as *const RmMotorsCan}, |v| Arc::into_raw(v)) as *mut RmMotorsCan
}

macro_rules! generate_wrapper {
    ($func_name:ident, ($($param_name:ident: $param_type:ty),*), $return_type:ty) => {
        #[no_mangle]
        pub extern "C" fn $func_name(rm_motors_can: *mut RmMotorsCan, $($param_name: $param_type),*) -> $return_type {
            if rm_motors_can.is_null(){
                println!("Invalid handle (null pointer)");
                return -1 as $return_type;
            }

            let rm_motors_can: Arc<RmMotorsCan> = unsafe { Arc::from_raw(rm_motors_can as *const RmMotorsCan) }; // reconstitute the Arc temporarily to clone it
            let rm_motors_can_ref2 = Arc::clone(&rm_motors_can);
            std::mem::forget(rm_motors_can); // "forget" the Arc to avoid dropping it (since C++ still needs to reuse it)
            rm_motors_can::$func_name(rm_motors_can_ref2, $($param_name),*).map_or_else(|e| {eprintln!("{}", e); -1 as $return_type}, |v| v as $return_type)
        }
    };
}

generate_wrapper!(init_motor, (id: u8, motor_type: MotorType, mode: CmdMode), i32);
generate_wrapper!(cleanup,    (period_ms: u64), i32);
generate_wrapper!(run_once,   (), i32);
generate_wrapper!(set_cmd,    (id: u8, cmd: f64), i32);
generate_wrapper!(get_state,  (id: u8, field: FbField), f64);


#[link(name = "rm_motors_can_test_cpp")]
extern { fn rm_motors_can_test_cpp() -> i32; }
// TODO this is only here due to a bug in the cc crate preventing c++ in examples: https://github.com/rust-lang/cc-rs/issues/1206
pub unsafe fn cpp_example(){
    std::process::exit(rm_motors_can_test_cpp());
}
