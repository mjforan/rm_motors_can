use std::thread;
use gm6020_can::{CmdMode, Gm6020Can};

#[async_std::main]
async fn main() -> anyhow::Result<()> {

    let delay = std::time::Duration::from_secs(1);

    let mut gmc = Gm6020Can::default();
    gmc.init("can0");
    gmc.cmd_single(1, CmdMode::Voltage, 5_i16)?;
    thread::sleep(delay);
    gmc.cmd_single(1, CmdMode::Voltage, 0_i16)?;
    thread::sleep(delay);
    gmc.cmd_single(1, CmdMode::Voltage, -5_i16)?;
    thread::sleep(delay);
    gmc.cmd_single(1, CmdMode::Voltage, 0_i16)?;
    gmc.run();
    

    Ok(())
}
