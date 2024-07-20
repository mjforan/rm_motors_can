use gm6020_can::{CmdMode, Gm6020Can};
use std::ffi::CString;
use std::thread;

const RATE: u64 = 50; // Don't want it too high because CAN bus will run out of buffer
const PERIOD: u64 = (1.0f64/(RATE as f64))as u64;
const INC: u64 = 10;
const MAX: i16 = (gm6020_can::V_MAX)as i16 * 10;
fn main() {
    let ifname: std::ffi::CString = CString::new("can0").expect("CString::new failed");
    let gmc: *mut Gm6020Can = gm6020_can::init(ifname.as_ptr());
    gm6020_can::run(gmc, PERIOD);

        for voltage in (0 .. MAX+1).step_by(2) {
            gm6020_can::cmd_single(gmc, CmdMode::Voltage, 1_u8, voltage as f64 / 10f64);
            thread::sleep(std::time::Duration::from_millis(INC));
        }
        for voltage in (0 .. MAX).rev().step_by(2) {
            gm6020_can::cmd_single(gmc, CmdMode::Voltage, 1_u8, voltage as f64 / 10f64);
            thread::sleep(std::time::Duration::from_millis(INC));
        }
        for voltage in (-1*MAX .. 0).rev().step_by(2) {
            gm6020_can::cmd_single(gmc, CmdMode::Voltage, 1_u8, voltage as f64 / 10f64);
            thread::sleep(std::time::Duration::from_millis(INC));
        }
        for voltage in (-1*MAX+1 .. 1).step_by(2) {
            gm6020_can::cmd_single(gmc, CmdMode::Voltage, 1_u8, voltage as f64 / 10f64);
            thread::sleep(std::time::Duration::from_millis(INC));
        }
}

