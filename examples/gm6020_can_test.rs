use gm6020_can::{CmdMode, FbField, Gm6020Can};
use std::ffi::CString;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::sync::Arc;
use ctrlc;

//////
// Example showing how to use gm6020_can library.
//////
/*
cargo build --release --examples && ./target/release/examples/gm6020_can_test
*/

const RATE: u64 = 100;                            // Should be above 100Hz for motor to run smoothly
const INC: u64 = 10;                              // Time (ms) between commands in the for loops
const MAX: i16 = (gm6020_can::V_MAX)as i16 * 10;  // Need the 10x multiplier so we can easily increment in for loops (can't increment floats).
const ID: u8 = 1;                                 // Motor ID [1,7]
const FB_FIELD: FbField = FbField::Velocity;      // The feedback value to visualize

fn main() {
    // Open SocketCAN device
    let ifname: std::ffi::CString = CString::new("can0").expect("CString::new failed");
    let gmc_: *mut Gm6020Can = gm6020_can::gm6020_can_init(ifname.as_ptr());
    if gmc_.is_null(){
        panic!("Unable to open specified SocketCAN device");
    }

    // Start another thread to collect feedback values and write commands
    // The weird variable `run_stop` points to an Arc<AtomicBool> just like the shared_stop below, but the data type is
    // an opaque pointer void* because C/C++ don't know about Arc<AtomicBool>
    let run_stop_raw: *mut std::ffi::c_void = gm6020_can::gm6020_can_run(gmc_, (1.0f64/(RATE as f64)*1000.0)as u64);
    // Need to convert it so Rust can "move" it into the ctrlc closure
    if unsafe.is_null() {panic!("run_stop_raw was nullptr");}
    let run_stop: &mut Arc<AtomicBool> = unsafe{&mut *(run_stop_raw as *mut Arc<AtomicBool>)}; // Wrap the raw pointer into Rust object

    // Start another thread to print current values
    let shared_stop: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    let shared_stop_ref2: Arc<AtomicBool> = shared_stop.clone();
    let shared_stop_ref3: Arc<AtomicBool> = shared_stop.clone();

    if gmc_.is_null(){
        panic!("Invalid handle (null pointer)");
    }
    // Breaks Rust's ownership rules because we will "keep" one in this scope, "move" one into the debug thread, and "move" one to the ctl-c handler
    // TODO maybe change this when implementing better multithreading protections
    let handle: &mut Gm6020Can = unsafe{&mut *gmc_}; // Wrap the raw pointer into Rust object so it can be "moved" to other contexts
    let handle2: &mut Gm6020Can = unsafe{&mut *gmc_};

    // Spawn another thread to visualize the feedback values
    let _dbg: JoinHandle<()> = thread::spawn( move ||
        while ! shared_stop_ref2.load(Ordering::Relaxed){
            thread::sleep(std::time::Duration::from_millis(50));
            print_output(handle);
        }
    );

    // Set up a signal handler to clean up (not strictly necessary but good practice)
    let _ = ctrlc::set_handler(move || {
        println!("cleaning up");
        // stop the printing thread:
        shared_stop.store(true, Ordering::Relaxed);
        gm6020_can::gm6020_can_cleanup(handle2, Box::into_raw(Box::new(run_stop.clone())) as *mut std::ffi::c_void);
    });

    // Ramp up, ramp down, ramp up (negative), ramp down (negative)
    for voltage in (0 .. MAX+1).step_by(2) {
        if shared_stop_ref3.load(Ordering::Relaxed) {return;} // Check if the ctl-c handler was called
        gm6020_can::gm6020_can_set_cmd(gmc_, ID, CmdMode::Voltage, voltage as f64 / 10f64);
        thread::sleep(std::time::Duration::from_millis(INC));
    }
    for voltage in (0 .. MAX).rev().step_by(2) {
        if shared_stop_ref3.load(Ordering::Relaxed) {return;} // Check if the ctl-c handler was called
        gm6020_can::gm6020_can_set_cmd(gmc_, ID, CmdMode::Voltage, voltage as f64 / 10f64);
        thread::sleep(std::time::Duration::from_millis(INC));
    }
    for voltage in (-1*MAX .. 0).rev().step_by(2) {
        if shared_stop_ref3.load(Ordering::Relaxed) {return;} // Check if the ctl-c handler was called
        gm6020_can::gm6020_can_set_cmd(gmc_, ID, CmdMode::Voltage, voltage as f64 / 10f64);
        thread::sleep(std::time::Duration::from_millis(INC));
    }
    for voltage in (-1*MAX+1 .. 1).step_by(2) {
        if shared_stop_ref3.load(Ordering::Relaxed) {return;} // Check if the ctl-c handler was called
        gm6020_can::gm6020_can_set_cmd(gmc_, ID, CmdMode::Voltage, voltage as f64 / 10f64);
        thread::sleep(std::time::Duration::from_millis(INC));
    }

    // Send constant voltage command
    gm6020_can::gm6020_can_set_cmd(gmc_, ID, CmdMode::Voltage, 2f64);
    while ! shared_stop_ref3.load(Ordering::Relaxed){
        thread::sleep(std::time::Duration::from_millis(50));
    }
}

// Print out a simple bar chart of feedback values
fn print_output(handle: &mut Gm6020Can) {
    let val = gm6020_can::gm6020_can_get_state(handle, ID, FB_FIELD);
    print!("{:.3}\t", val);
    match FB_FIELD {
        FbField::Position    => println!("{:#<1$}", "", (val*5f64) as usize),
        FbField::Velocity    => println!("{:#<1$}", "", val.abs() as usize),
        FbField::Current     => println!("{:#<1$}", "", (val.abs()*10f64) as usize),
        FbField::Temperature => println!("{:#<1$}", "", val as usize),
    }
}