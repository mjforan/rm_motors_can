use std::thread;
use gm6020_can::{CmdMode, Gm6020Can};

fn main() {

    let delay = std::time::Duration::from_secs(1);

    let gmc: *mut Gm6020Can = gm6020_can::init("can0");
    thread::spawn(gm6020_can::run(gmc));
    gm6020_can::cmd_single(gmc, CmdMode::Voltage, 0_u8, 5_f64);
    thread::sleep(delay);
    gm6020_can::cmd_single(gmc, CmdMode::Voltage, 0_u8, 0_f64);
    thread::sleep(delay);
    gm6020_can::cmd_single(gmc, CmdMode::Voltage, 0_u8, -5_f64);
    thread::sleep(delay);
    gm6020_can::cmd_single(gmc, CmdMode::Voltage, 0_u8, 0_f64);

    Ok(())
}
