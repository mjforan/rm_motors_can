use anyhow::Context;
use embedded_can::{blocking::Can, Frame as EmbeddedFrame, StandardId};
use socketcan::{CanFrame, CanSocket, Frame, Socket};
use std::env;

fn main() -> anyhow::Result<()> {
    let iface = env::args().nth(1).unwrap_or_else(|| "can0".into());

    let mut sock = CanSocket::open(&iface)
        .with_context(|| format!("Failed to open socket on interface {}", iface))?;

    let frame = sock.receive().context("Receiving frame")?;

    println!("{}  {}", iface, frame_to_string(&frame));

    let frame = CanFrame::new(StandardId::new(0x1f1).unwrap(), &[1, 2, 3, 4])
        .context("Creating CAN frame")?;

    sock.transmit(&frame).context("Transmitting frame")?;

    Ok(())
}

fn frame_to_string<F: Frame>(frame: &F) -> String {
    let id = frame.raw_id();
    let data_string = frame
        .data()
        .iter()
        .fold(String::from(""), |a, b| format!("{} {:02x}", a, b));

    format!("{:X}  [{}] {}", id, frame.dlc(), data_string)
}







const FB_ID_BASE: u16 = 0x204;
const CMD_ID_V_L: u16 = 0x1ff;
const CMD_ID_V_H: u16 = 0x2ff;
const CMD_ID_I_L: u16 = 0x1fe;
const CMD_ID_I_H: u16 = 0x2fe;
const ID_MIN: u8= 1;
const ID_MAX: u8 = 7;

const RPM_PER_V  : f64 =  13.33;
const N_PER_A    : f64 = 741;
const I_MAX      : f64 =   1.62;
const RPM_PER_N_M: f64 = 156;
const TEMP_MAX   : f64 = 125; // C

struct Feedback {
    position: u16, // [0, 8191]
    speed: i16,    // rpm
    current: i16,  // [-16384, 16384]:[-3A, 3A]
    temp: u8,      // TODO units
}
struct Command {
    voltage: i16,
    current: i16,    
}

let feedbacks: [Feedback; ID_MAX-ID_MIN+1];
let commands: [Command; ID_MAX-ID_MIN+1];

fn cmd_voltage_single(id: u8, voltage: i16) /*TODO -> possibly error*/ {
    // TODO check id range
    commands[id-1].voltage = voltage;
    let id_range: bool = id > 4;
    let offset = 4 * (id_range as u8);
    tx_cmd_voltages(id_range, commands[offset+0], commands[offset+1], commands[offset+2], commands[offset+3]);
}

// TODO input types
fn cmd_voltage_multiple(commands: [(id: u8; voltage: i16); ??]) /*TODO -> possibly error*/ {
    mut send_low: bool = false;
    mut send_high: bool = false;
    //TODO for loop
    for cmd in commands{
        send_low |= cmd.id<=4;    
        send_high |= cmd.id>4;
        commands[cmd.id-1].voltage = cmd.voltage;
    }
    if send_low { tx_cmd_voltages(false); }
    if send_high { tx_cmd_voltages(true); }
}


// Send multiple voltages at once for more efficient communication
fn tx_cmd_voltages(id_range: bool) /*TODO -> possibly error*/ {
    let id: u16 = match id_range { 0 => CMD_ID_V_L, 1 => CMD_ID_V_H };
    // TODO check temp before sending
}
/**
  * @brief  send motor control message through can bus
  * @param  id_range to select can control id 0x1ff or 0x2ff
  * @param  motor voltage 1,2,3,4 or 5,6,7
  * @retval None
  */
void set_motor_voltage(uint8_t id_range, int16_t v1, int16_t v2, int16_t v3, int16_t v4)
{
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
}






void can_user_init(CAN_HandleTypeDef* hcan);
void set_motor_voltage(uint8_t id_range, int16_t v1, int16_t v2, int16_t v3, int16_t v4);


moto_info_t motor_info[MOTOR_MAX_NUM];

/**
  * @brief  init can filter, start can, enable can rx interrupt
  * @param  hcan pointer to a CAN_HandleTypeDef structure that contains
  *         the configuration information for the specified CAN.
  * @retval None
  */
void can_user_init(CAN_HandleTypeDef* hcan )
{
  CAN_FilterTypeDef  can_filter;

  can_filter.FilterBank = 0;                       // filter 0
  can_filter.FilterMode =  CAN_FILTERMODE_IDMASK;  // mask mode
  can_filter.FilterScale = CAN_FILTERSCALE_32BIT;
  can_filter.FilterIdHigh = 0;
  can_filter.FilterIdLow  = 0;
  can_filter.FilterMaskIdHigh = 0;
  can_filter.FilterMaskIdLow  = 0;                // set mask 0 to receive all can id
  can_filter.FilterFIFOAssignment = CAN_RX_FIFO0; // assign to fifo0
  can_filter.FilterActivation = ENABLE;           // enable can filter
  can_filter.SlaveStartFilterBank  = 14;          // only meaningful in dual can mode
   
  HAL_CAN_ConfigFilter(hcan, &can_filter);        // init can filter
  HAL_CAN_Start(&hcan1);                          // start can1
  HAL_CAN_ActivateNotification(&hcan1, CAN_IT_RX_FIFO0_MSG_PENDING); // enable can1 rx interrupt
}

/**
  * @brief  can rx callback, get motor feedback info
  * @param  hcan pointer to a CAN_HandleTypeDef structure that contains
  *         the configuration information for the specified CAN.
  * @retval None
  */
void HAL_CAN_RxFifo0MsgPendingCallback(CAN_HandleTypeDef *hcan)
{
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
}

