use gm6020_can::{CmdMode, FbField, Gm6020Can};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::sync::Arc;
use ctrlc;

//////
// Example showing how to use gm6020_can library.
//////
/*
cargo run --example gm6020_can_test
*/

const INC: u64 = 10;                               // Time (ms) between commands in the for loops
const MAX: i16 = (gm6020_can::V_MAX) as i16 * 10;  // Need the 10x multiplier so we can easily increment in for loops (can't increment floats).
const ID: u8 = 1;                                  // Motor ID [1,7]
const FB_FIELD: FbField = FbField::Velocity;       // The feedback value to visualize
const CAN_INTERFACE: &str = "can0";                // SocketCAN interface to open

fn main() {
    // Open SocketCAN device
    let gmc: Arc<Gm6020Can> = gm6020_can::init(CAN_INTERFACE).unwrap();
    
    // Atomic flag to trigger stopping the threads
    let shared_stop: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));

    // Spawn another thread to visualize the feedback values
    let shared_stop_ref2: Arc<AtomicBool> = shared_stop.clone();
    let gmc_ref2: Arc<Gm6020Can> = gmc.clone();
    let _dbg: JoinHandle<()> = thread::spawn( move ||
        while ! shared_stop_ref2.load(Ordering::Relaxed){
            thread::sleep(std::time::Duration::from_millis(50));
            print_output(gmc_ref2.clone());
        }
    );

    // Set up a signal handler to clean up (not strictly necessary but good practice)
    let shared_stop_ref3: Arc<AtomicBool> = shared_stop.clone();
    let shared_final: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    let shared_final_ref2: Arc<AtomicBool> = shared_final.clone();
    let gmc_ref3: Arc<Gm6020Can> = gmc.clone();
    let _ = ctrlc::set_handler(move || {
        // stop the other threads
        shared_stop_ref3.store(true, Ordering::Relaxed);
        // gently turn off the motors
        gm6020_can::cleanup(gmc_ref3.clone(), 5).map_or_else(|e| eprintln!("{}", e), |_| ());
        shared_final_ref2.store(true, Ordering::Relaxed);
    });

    // Start another thread to periodically collect feedbacks and write commands
    // It's better to run_once() after every set_cmd to minimize delay before writing,
    // but if this loop is fast enough it will not be noticeable. This approach has the advantage of
    // running consistently, which prevents the socket buffer from filling up in case e.g. the main thread is blocked.
    let shared_stop_ref4: Arc<AtomicBool> = shared_stop.clone();
    let gmc_ref4: Arc<Gm6020Can> = gmc.clone();
    thread::spawn( move ||
        while ! shared_stop_ref4.load(Ordering::Relaxed) {
            gm6020_can::run_once(gmc_ref4.clone()).map_or_else(|e| eprintln!("{}", e), |_| ());
            thread::sleep(std::time::Duration::from_millis(INC));
        }
    );

    // Ramp up, ramp down, ramp up (negative), ramp down (negative)
    for voltage in (0 .. MAX+1).step_by(2) {
        if shared_stop.load(Ordering::Relaxed) {break;} // Check if the ctl-c handler was called
        gm6020_can::set_cmd(gmc.clone(), ID, CmdMode::Voltage, voltage as f64 / 10f64).map_or_else(|e| eprintln!("{}", e), |_| ());
        thread::sleep(std::time::Duration::from_millis(INC));
    }
    for voltage in (0 .. MAX).rev().step_by(2) {
        if shared_stop.load(Ordering::Relaxed) {break;} // Check if the ctl-c handler was called
        gm6020_can::set_cmd(gmc.clone(), ID, CmdMode::Voltage, voltage as f64 / 10f64).map_or_else(|e| eprintln!("{}", e), |_| ());
        thread::sleep(std::time::Duration::from_millis(INC));
    }
    for voltage in (-1*MAX .. 0).rev().step_by(2) {
        if shared_stop.load(Ordering::Relaxed) {break;} // Check if the ctl-c handler was called
        gm6020_can::set_cmd(gmc.clone(), ID, CmdMode::Voltage, voltage as f64 / 10f64).map_or_else(|e| eprintln!("{}", e), |_| ());
        thread::sleep(std::time::Duration::from_millis(INC));
    }
    for voltage in (-1*MAX+1 .. 1).step_by(2) {
        if shared_stop.load(Ordering::Relaxed) {break;} // Check if the ctl-c handler was called
        gm6020_can::set_cmd(gmc.clone(), ID, CmdMode::Voltage, voltage as f64 / 10f64).map_or_else(|e| eprintln!("{}", e), |_| ());
        thread::sleep(std::time::Duration::from_millis(INC));
    }

    // Send constant voltage command
    gm6020_can::set_cmd(gmc.clone(), ID, CmdMode::Voltage, 2f64).map_or_else(|e| eprintln!("{}", e), |_| ());
    
    // Wait for the ctl-c handler to finish cleaning up
    while ! shared_final.load(Ordering::Relaxed){
        thread::sleep(std::time::Duration::from_millis(50));
    }
}

// Print out a simple bar chart of feedback values
fn print_output(gm6020_can: Arc<Gm6020Can>) {
    let val = gm6020_can::get_state(gm6020_can, ID, FB_FIELD).unwrap();
    println!("{:#<1$}", "", match FB_FIELD {
        FbField::Position    => (val*5f64) as usize,
        FbField::Velocity    => val.abs() as usize,
        FbField::Current     => (val.abs()*10f64) as usize,
        FbField::Temperature => val as usize,
    })
}