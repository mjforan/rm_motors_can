use gm6020_can::*;

use std::ffi::CStr;
use std::os::raw::c_char;
use std::ptr::null;
use std::sync::Arc;

// TODO how to get constants in cbindgen header
pub const V_MAX: f64 = 24.0; //gm6020_can::V_MAX;


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


/*
**  Clean up pointers and release the socket
**  gm6020_can: 'handle' to act upon
**  run_stopper: raw pointer to Arc<AtomicBool> which is shared with the run thread
*/
#[no_mangle]
pub extern "C" fn cleanup(gm6020_can: *mut Gm6020Can, period_ms: u64){
    if gm6020_can.is_null(){
        println!("Invalid handle (null pointer)");
        return;
    }
    // Rebuild the Arc temporarily to clone it
    let arc_ref: Arc<Gm6020Can> = unsafe { Arc::from_raw(gm6020_can as *const Gm6020Can) };

    // Clone the Arc to increase the reference count
    let arc_clone = Arc::clone(&arc_ref);
    
    // Forget the original Arc to avoid dropping it (since C++ still holds it)
    std::mem::forget(arc_ref);
    gm6020_can::cleanup(arc_clone, period_ms);
}



/*
** Update motor feedbacks and send commands.
**
**  gm6020_can: 'handle' to act upon
**  returns: 0 on success, -1 otherwise
*/
#[no_mangle]
pub extern "C" fn run_once(gm6020_can: *mut Gm6020Can) -> i8{
    if gm6020_can.is_null(){
        println!("Invalid handle (null pointer)");
        return -1;
    }
    // Rebuild the Arc temporarily to clone it
    let arc_ref: Arc<Gm6020Can> = unsafe { Arc::from_raw(gm6020_can as *const Gm6020Can) };

    // Clone the Arc to increase the reference count
    let arc_clone = Arc::clone(&arc_ref);
    
    // Forget the original Arc to avoid dropping it (since C++ still holds it)
    std::mem::forget(arc_ref);
    gm6020_can::run_once(arc_clone).map_or_else(|e| {eprintln!("{}", e); -1_i8}, |_| 0_i8)
}


/*
** Convert various motor command types into the expected low-level format
**
**  gm6020_can: 'handle' to act upon
**  id: motor ID
**  mode: what type of command is being written
**  cmd: actual command value
*/
#[no_mangle]
pub extern "C" fn set_cmd(gm6020_can: *mut Gm6020Can, id: u8, mode: CmdMode, cmd: f64) -> i8{
    if gm6020_can.is_null(){
        println!("Invalid handle (null pointer)");
        return -1;
    }
    // Rebuild the Arc temporarily to clone it
    let arc_ref: Arc<Gm6020Can> = unsafe { Arc::from_raw(gm6020_can as *const Gm6020Can) };

    // Clone the Arc to increase the reference count
    let arc_clone = Arc::clone(&arc_ref);
    
    // Forget the original Arc to avoid dropping it (since C++ still holds it)
    std::mem::forget(arc_ref);
    gm6020_can::set_cmd(arc_clone, id, mode, cmd).map_or_else(|e| {eprintln!("{}", e); -1_i8}, |_| 0_i8)
}



/*
**  Convert a motor's low-level feedback into normal units
**
**  gm6020_can: 'handle' to act upon
**  id: motor ID
**  field: the feedback item to get
*/
#[no_mangle]
pub extern "C" fn get_state(gm6020_can: *mut Gm6020Can, id: u8, field: FbField) -> f64{
    if gm6020_can.is_null(){
        println!("Invalid handle (null pointer)");
        return f64::NAN;
    }
    // Rebuild the Arc temporarily to clone it
    let arc_ref: Arc<Gm6020Can> = unsafe { Arc::from_raw(gm6020_can as *const Gm6020Can) };

    // Clone the Arc to increase the reference count
    let arc_clone = Arc::clone(&arc_ref);
    
    // Forget the original Arc to avoid dropping it (since C++ still holds it)
    std::mem::forget(arc_ref);
    gm6020_can::get_state(arc_clone, id, field)
}




#[link(name = "gm6020_can_test_cpp")]
extern { fn gm6020_can_test_cpp() -> i32; }
// TODO this is only here due to a bug in the cc crate preventing c++ in examples: https://github.com/rust-lang/cc-rs/issues/1206
pub unsafe fn cpp_example(){
    std::process::exit(gm6020_can_test_cpp());
}
