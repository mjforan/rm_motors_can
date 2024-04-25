use anyhow::{Context, anyhow};
use embedded_can::{blocking::Can, Frame as EmbeddedFrame, StandardId};
use socketcan::{CanFrame, CanSocket, Socket};
use std::thread;
use std::time::{SystemTime};

const FB_ID_BASE: u16 = 0x204;
const CMD_ID_V_L: u16 = 0x1ff;
const CMD_ID_V_H: u16 = 0x2ff;
const CMD_ID_I_L: u16 = 0x1fe;
const CMD_ID_I_H: u16 = 0x2fe;
const ID_MIN: u8= 1;
const ID_MAX: u8 = 7;

const RPM_PER_V  : f64 =  13.33;
const N_PER_A    : f64 = 741.0;
const I_MAX      : f64 =   1.62;
const RPM_PER_N_M: f64 = 156.0;
const TEMP_MAX   : f64 = 125.0; // C

#[derive(Default, Copy, Clone)]
pub enum CmdMode { #[default] Voltage, Current }
#[derive(Default)]
enum IdRange { #[default] Low, High }

#[derive(Default)]
struct Feedback {
    timestamp: Option<SystemTime>,
    position: u16, // [0, 8191]
    speed: i16,    // rpm
    current: i16,  // [-16384, 16384]:[-3A, 3A]
    temp: u8,      // TODO units
}
#[derive(Default)]
struct Command {
    mode: CmdMode,
    cmd: i16,
}
#[derive(Default)]
pub struct Gm6020Can {
    socket: Option<CanSocket>,
    feedbacks: [Feedback; (ID_MAX-ID_MIN+1) as usize],
    commands: [Command; (ID_MAX-ID_MIN+1)as usize],
}

impl Gm6020Can {
    pub fn init(&mut self, interface: &str) -> anyhow::Result<()> {
        self.socket = Some(CanSocket::open(&interface)
            .with_context(|| format!("Failed to open socket on interface {}", interface))?);

    //    let frame = sock.receive().context("Receiving frame")?;

    //    println!("{}  {}", iface, frame_to_string(&frame));
        // TODO set filter for feedback IDs and set async subscriber to update feedbacks array
        Ok(())
    }

    pub fn cmd_single(&mut self, id: u8, mode: CmdMode, cmd: i16) -> anyhow::Result<()> {
        if id<ID_MIN || id>ID_MAX { return Err(anyhow!("id out of range [{}, {}]: {}", ID_MIN, ID_MAX, id)); }
        self.commands[(id-1) as usize].mode = mode;
        self.commands[(id-1) as usize].cmd = cmd;
        self.tx_cmd(match id > 4 {false=>IdRange::Low, true=>IdRange::High}, mode).context("transmitting commands")?;
        Ok(())
    }

    pub fn cmd_multiple(&mut self, mode: CmdMode, commands: Vec<(u8, i16)> ) -> anyhow::Result<()> {
        let mut send_low: bool = false;
        let mut send_high: bool = false;
        for cmd in commands.into_iter(){
            send_low |= cmd.0<=4;    
            send_high |= cmd.0>4;
            self.commands[(cmd.0-1) as usize].mode = mode;
            self.commands[(cmd.0-1) as usize].cmd = cmd.1;
        }
        if send_low { self.tx_cmd(IdRange::Low, mode).context("transmitting commands")?; }
        if send_high { self.tx_cmd(IdRange::High, mode).context("transmitting commands")?; }
        Ok(())
    }


    // Send multiple voltages at once for more efficient communication
    fn tx_cmd(&mut self, id_range: IdRange, mode: CmdMode) -> anyhow::Result<()> {
        let id: u16 = match id_range { IdRange::Low => CMD_ID_V_L, IdRange::High => CMD_ID_V_H };
        // TODO check temp before sending - put warning output when maxed
        let frame = CanFrame::new(StandardId::new(0x1f1).unwrap(), &[1, 2, 3, 4])
            .context("Creating CAN frame")?;

        match &mut self.socket{
            Some(sock) => sock.transmit(&frame).context("Transmitting frame")?,
            None => return Err(anyhow!("Socket not initialized")),
        }
    /*
      CAN_TxHeaderTypeDef tx_header;
      uint8_t             tx_data[8];
        
      tx_header.StdId = (id_range == 0)?(0x1ff):(0x2ff);
      tx_header.IDE   = CAN_ID_STD;
      tx_header.RTR   = CAN_RTR_DATA;
      tx_header.DLC   = 8;

      tx_data[0] = (v1>>8)&0xff;
      tx_data[1] =    (v1)&0xff;
      tx_data[2] = (v2>>8)&0xff;
      tx_data[3] =    (v2)&0xff;
      tx_data[4] = (v3>>8)&0xff;
      tx_data[5] =    (v3)&0xff;
      tx_data[6] = (v4>>8)&0xff;
      tx_data[7] =    (v4)&0xff;
      HAL_CAN_AddTxMessage(&hcan1, &tx_header, tx_data,(uint32_t*)CAN_TX_MAILBOX0); 
    */

        Ok(())
    }

    fn rx_fb(&mut self, frame: CanFrame) -> anyhow::Result<()> {
        let stamp = SystemTime::now(); // TODO waiting on socketcan library to implement hardware timestamps
/*
  CAN_RxHeaderTypeDef rx_header;
  uint8_t             rx_data[8];
  if(hcan->Instance == CAN1)
  {
    HAL_CAN_GetRxMessage(hcan, CAN_RX_FIFO0, &rx_header, rx_data); //receive can data
  }
  if ((rx_header.StdId >= FEEDBACK_ID_BASE)
   && (rx_header.StdId <  FEEDBACK_ID_BASE + MOTOR_MAX_NUM))                  // judge the can id
  {
    uint8_t index = rx_header.StdId - FEEDBACK_ID_BASE;                  // get motor index by can_id
    motor_info[index].rotor_angle    = ((rx_data[0] << 8) | rx_data[1]);
    motor_info[index].rotor_speed    = ((rx_data[2] << 8) | rx_data[3]);
    motor_info[index].torque_current = ((rx_data[4] << 8) | rx_data[5]);
    motor_info[index].temp           =   rx_data[6];
  }
*/

        let id: u8 = 1;
        let f: &mut Feedback = &mut self.feedbacks[(id-1) as usize];
        f.timestamp = Some(stamp);
        Ok(())
    }
}


fn main() -> anyhow::Result<()> {

    let delay = std::time::Duration::from_secs(1);

    let mut gmc = Gm6020Can::default();
    gmc.cmd_single(1, CmdMode::Voltage, 5_i16)?;
    thread::sleep(delay);
    gmc.cmd_single(1, CmdMode::Voltage, 0_i16)?;
    thread::sleep(delay);
    gmc.cmd_single(1, CmdMode::Voltage, -5_i16)?;
    thread::sleep(delay);
    gmc.cmd_single(1, CmdMode::Voltage, 0_i16)?;
    

    Ok(())
}


