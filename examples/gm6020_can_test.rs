use gm6020_can::{CmdMode, Gm6020Can};
use std::ffi::CString;
use std::thread;



const RATE: u64 = 100;
const PERIOD: u64 = (1.0f32/(RATE as f32))as u64;
fn main() {
    let ifname: std::ffi::CString = CString::new("can0").expect("CString::new failed");
    let gmc: *mut Gm6020Can = gm6020_can::init(ifname.as_ptr());

    gm6020_can::cmd_single(gmc, CmdMode::Voltage, 1_u8, 5_f64);
    gm6020_can::run(gmc, PERIOD);
    thread::sleep(std::time::Duration::from_secs(2));
    gm6020_can::cmd_single(gmc, CmdMode::Voltage, 1_u8, 10_f64);
    thread::sleep(std::time::Duration::from_secs(2));

    gm6020_can::cmd_single(gmc, CmdMode::Voltage, 1_u8, 5_f64);
    thread::sleep(std::time::Duration::from_secs(2));

    gm6020_can::cmd_single(gmc, CmdMode::Voltage, 1_u8, 0_f64);
}
