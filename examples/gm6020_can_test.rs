use gm6020_can::{CmdMode, Gm6020Can};
use std::ffi::CString;
use std::thread;

const RATE: u64 = 100;
const PERIOD: u64 = (1.0f64/(RATE as f64))as u64;
const INC: u64 = 10;
const MAX: i16 = 240;
fn main() {
    let ifname: std::ffi::CString = CString::new("can0").expect("CString::new failed");
    let gmc: *mut Gm6020Can = gm6020_can::init(ifname.as_ptr());
    gm6020_can::run(gmc, PERIOD);

        for voltage in 0 .. MAX+1 {
            gm6020_can::cmd_single(gmc, CmdMode::Voltage, 1_u8, voltage as f64 / 10f64);
            thread::sleep(std::time::Duration::from_millis(INC));
        }
        for voltage in (0 .. MAX).rev() {
            gm6020_can::cmd_single(gmc, CmdMode::Voltage, 1_u8, voltage as f64 / 10f64);
            thread::sleep(std::time::Duration::from_millis(INC));
        }
        for voltage in (-1*MAX .. 0).rev() {
            gm6020_can::cmd_single(gmc, CmdMode::Voltage, 1_u8, voltage as f64 / 10f64);
            thread::sleep(std::time::Duration::from_millis(INC));
        }
        for voltage in -1*MAX+1 .. 1 {
            gm6020_can::cmd_single(gmc, CmdMode::Voltage, 1_u8, voltage as f64 / 10f64);
            thread::sleep(std::time::Duration::from_millis(INC));
        }
}

