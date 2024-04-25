use anyhow::Context;
use embedded_can::{blocking::Can, Frame as EmbeddedFrame, StandardId};
use socketcan::{CanFrame, CanSocket, Socket};

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

pub struct Gm6020Can {
    socket: CanSocket,
    feedbacks: [Feedback; (ID_MAX-ID_MIN+1) as usize],
    commands: [Command; (ID_MAX-ID_MIN+1)as usize],
}

impl Gm6020Can {
    pub fn init(&mut self, interface: String) -> anyhow::Result<()> {
        self.socket = CanSocket::open(&interface)
            .with_context(|| format!("Failed to open socket on interface {}", interface))?;

    //    let frame = sock.receive().context("Receiving frame")?;

    //    println!("{}  {}", iface, frame_to_string(&frame));

        Ok(())
    }

    pub fn cmd_voltage_single(&mut self, id: u8, voltage: i16) -> anyhow::Result<()> {
        if id<ID_MIN || id>ID_MAX { return Err(()); }

        self.commands[id-1].voltage = voltage;
        let id_range: u8 = (id > 4) as u8;
        self.tx_cmd_voltages(id_range).context("transmitting voltage commands")?;
        Ok();
    }

    // TODO input types
    pub fn cmd_voltage_multiple(&mut self, commands: Vec<(u8, i16)> ) -> anyhow::Result<()> {
        let mut send_low: bool = false;
        let mut send_high: bool = false;
        //TODO for loop
        for cmd in commands.into_iter(){
            send_low |= cmd.0<=4;    
            send_high |= cmd.0>4;
            self.commands[cmd.id-1].voltage = cmd.1;
        }
        if send_low { self.tx_cmd_voltages(0_u8).context("transmitting voltage commands")?; }
        if send_high { self.tx_cmd_voltages(1_u8).context("transmitting voltage commands")?; }
        Ok(())
    }


    // Send multiple voltages at once for more efficient communication
    fn tx_cmd_voltages(&mut self, id_range: u8) -> anyhow::Result<()> {
        let id: u16 = match id_range { 0 => CMD_ID_V_L, 1 => CMD_ID_V_H };
        // TODO check temp before sending - put warning output when maxed
        let frame = CanFrame::new(StandardId::new(0x1f1).unwrap(), &[1, 2, 3, 4])
            .context("Creating CAN frame")?;

        self.socket.transmit(&frame).context("Transmitting frame")?;
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

}


fn main() -> anyhow::Result<()> {

    Ok(())
}









/*
moto_info_t motor_info[MOTOR_MAX_NUM];

*
  * @brief  init can filter, start can, enable can rx interrupt
  * @param  hcan pointer to a CAN_HandleTypeDef structure that contains
  *         the configuration information for the specified CAN.
  * @retval None
  
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

**
  * @brief  can rx callback, get motor feedback info
  * @param  hcan pointer to a CAN_HandleTypeDef structure that contains
  *         the configuration information for the specified CAN.
  * @retval None
  
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

*/
