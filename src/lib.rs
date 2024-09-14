use gm6020_can::*;

use std::ffi::CStr;
use std::os::raw::c_char;
use std::sync::atomic::{AtomicBool, Ordering};
use std::ptr::null;
use std::sync::Arc;

// TODO handle CAN no buffer left
// TODO try to reduce panics and handle errors more gracefully

/*
**  interface: SocketCAN interface name e.g. "can0"
**  returns: pointer to Gm6020Can struct, to be passed to other functions in this library
*/
#[no_mangle]
pub extern "C" fn init(interface: *const c_char) -> *mut std::ffi::c_void {
    let inter: &str;
    if interface.is_null() {
        println!("Invalid c-string received for interface name (null pointer)");
        return null::<Gm6020Can>() as *mut std::ffi::c_void;
    }
    else {
        unsafe {
            let r = CStr::from_ptr(interface).to_str();
            if r.is_err() {
                eprintln!("Invalid c-string received for interface name");
                return null::<Gm6020Can>() as *mut std::ffi::c_void;
            }
            inter = r.unwrap();
        }
    }
    gm6020_can::init(inter).map_or_else(|e| {eprintln!("{}", e); null::<Gm6020Can>() as *mut std::ffi::c_void}, |v| Box::into_raw(v) as *mut std::ffi::c_void)
}


/*
**  Clean up pointers and release the socket
**  gm6020_can: 'handle' to act upon
**  run_stopper: raw pointer to Arc<AtomicBool> which is shared with the run thread
*/
#[no_mangle]
pub extern "C" fn cleanup(gm6020_can: *mut std::ffi::c_void, run_stopper: *mut std::ffi::c_void){
    if gm6020_can.is_null(){
        println!("Invalid handle (null pointer)");
        return;
    }
    // TODO re-enable this after figuring out multithreading stuff
    // Ramp down commands to avoid jerking stop
    /*
    let handle: &mut Gm6020Can = unsafe{&mut *gm6020_can}; // Wrap the raw pointer into Rust object

    for i in 0 .. handle.modes.len() {
        let m: CmdMode = handle.modes[i];
        if m == CmdMode::Disabled {continue;}
        thread::spawn( move ||{
            let mut cmd: f64 = match m {
                CmdMode::Voltage => handle.commands[i] as f64 /V_CMD_MAX*V_MAX,
                CmdMode::Current => handle.commands[i] as f64 /I_CMD_MAX*I_MAX,
            };
            let sign: f64 = cmd/cmd.abs();
            loop{
                if cmd <= 0.2 {
                    gm6020_can_set_cmd(gm6020_can, i as u8+ID_MIN, m, 0f64);
                    gm6020_canrun_once(gm6020_can);
                    break;
                }
                cmd = cmd-sign*0.2;
                gm6020_can_set_cmd(gm6020_can, i as u8+ID_MIN, m, cmd);
                gm6020_canrun_once(gm6020_can);
                thread::sleep(std::time::Duration::from_millis(5));                
            }
        });
    }*/

    if gm6020_can.is_null(){
        eprintln!("Invalid handle (null pointer)");
    }
    else{
        unsafe{drop(Box::from_raw(gm6020_can));}// Delete the pointer. The socket is automatically closed when all references are dropped.
    }
    if run_stopper.is_null() {
        eprintln!("Invalid run_stopper (null pointer)");
    }
    else{
        let stop: &mut Arc<AtomicBool> = unsafe{&mut *(run_stopper as *mut Arc<AtomicBool>)};
        stop.store(true, Ordering::Relaxed); // Stop the thread
        unsafe{drop(Box::from_raw(run_stopper))}; // Delete the pointer - actually just decreases the reference count, it won't be deleted until the thread also drops its reference.
    }
}



/*
** Update motor feedbacks and send commands.
**
**  gm6020_can: 'handle' to act upon
**  returns: 0 on success, -1 otherwise
*/
#[no_mangle]
pub extern "C" fn run_once(gm6020_can: *mut std::ffi::c_void) -> i8{
    if gm6020_can.is_null(){
        println!("Invalid handle (null pointer)");
        return -1;
    }
    let handle: &mut Gm6020Can = unsafe{&mut *(gm6020_can as *mut Gm6020Can)}; // Wrap the raw pointer into Rust object
    gm6020_can::run_once(handle).map_or_else(|e| {eprintln!("{}", e); -1_i8}, |_| 0_i8)
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
pub extern "C" fn set_cmd(gm6020_can: *mut std::ffi::c_void, id: u8, mode: CmdMode, cmd: f64) -> i8{
    let handle: &mut Gm6020Can;
    if gm6020_can.is_null(){
        println!("Invalid handle (null pointer)");
        return -1;
    }
    handle = unsafe{&mut *(gm6020_can as *mut Gm6020Can)}; // Wrap the raw pointer into Rust object
    gm6020_can::set_cmd(handle, id, mode, cmd).map_or_else(|e| {eprintln!("{}", e); -1_i8}, |_| 0_i8)
}



/*
**  Convert a motor's low-level feedback into normal units
**
**  gm6020_can: 'handle' to act upon
**  id: motor ID
**  field: the feedback item to get
*/
#[no_mangle]
pub extern "C" fn get_state(gm6020_can: *mut std::ffi::c_void, id: u8, field: FbField) -> f64{
    if id<ID_MIN || id>ID_MAX { panic!("id out of range [{}, {}]: {}", ID_MIN, ID_MAX, id);}
    let handle: &mut Gm6020Can;
    if gm6020_can.is_null(){
        panic!("Invalid handle");
    }
    handle = unsafe{&mut *(gm6020_can as *mut Gm6020Can)}; // Wrap the raw pointer into Rust object
    gm6020_can::get_state(handle, id, field)
}




#[link(name = "gm6020_can_test_cpp")]
extern { fn test_cpp(); }
// TODO this is only here due to a bug in the cc crate preventing c++ in examples: https://github.com/rust-lang/cc-rs/issues/1206
pub unsafe fn cpp_example(){
    test_cpp();
}
