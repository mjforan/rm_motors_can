use std::fmt;
use socketcan::{CanDataFrame, CanFilter, CanFrame, CanSocket, Frame, Socket, SocketOptions};
use std::f64::consts::PI;
use std::time::Duration;
use embedded_can::{Frame as EmbeddedFrame, StandardId};
use std::time::SystemTime;

const FB_ID_BASE: u16 = 0x204;
const CMD_ID_V_L: u16 = 0x1ff;
const CMD_ID_V_H: u16 = 0x2ff;
const CMD_ID_I_L: u16 = 0x1fe;
const CMD_ID_I_H: u16 = 0x2fe;
pub const ID_MIN: u8 = 1;
pub const ID_MAX: u8 = 7;
const POS_MAX   : u16 = 8191;

pub const RPM_PER_ANGULAR : f64 = 60.0/(2.0*3.14159);
pub const RPM_PER_V : f64 =  13.33;
pub const NM_PER_A  : f64 =   0.741;
pub const V_MAX     : f64 =  24.0;
pub const I_MAX     : f64 =   1.62;
pub const TEMP_MAX  : u8  = 125; // C
pub const I_FB_MAX  : f64 =   3.0;
const V_CMD_MAX : f64 = 25000.0;
const I_CMD_MAX : f64 = 16384.0;


#[derive(Copy, Clone, PartialEq, Eq)]
#[repr(C)]
pub enum CmdMode { Disabled, Voltage, Current, Torque, Velocity }
impl Default for CmdMode {
    fn default() -> Self { CmdMode::Disabled }
}
impl fmt::Display for CmdMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CmdMode::Disabled => write!(f, "Disabled"),
            CmdMode::Voltage  => write!(f, "Voltage"),
            CmdMode::Current  => write!(f, "Current"),
            CmdMode::Torque   => write!(f, "Torque"),
            CmdMode::Velocity => write!(f, "Velocity"),
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
#[repr(C)]
pub enum FbField { Position, Velocity, Current, Temperature }
impl Default for FbField {
    fn default() -> Self { FbField::Position }
}

#[derive(Default, Debug)]
#[repr(C)]
struct Feedback {
    position:    u16, // [0, 8191]
    velocity:    i16, // rpm
    current:     i16, // [-16384, 16384]:[-3A, 3A]
    temperature: u16, // C
}

#[derive(Default)]
#[repr(C)]
pub struct Gm6020Can {
    socket: Option<CanSocket>,
    // (ID_MAX-ID_MIN+1) as usize   only 7 slots will be used but 8 is convenient for tx_cmd (last item unused)
    modes    : [CmdMode; 8], //todo rwlock these
    commands : [i16; 8],
    feedbacks: [(Option<SystemTime>, Feedback); 8],
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
enum IdRange { Low, High }
impl IdRange {
    fn from_u8(id: u8) -> IdRange {
        if id < ID_MIN || id > ID_MAX + 1 { // +1 to account for full range of array in tx_cmd, even though it's not technically a valid ID
            panic!("id out of range [{}, {}]: {}", ID_MIN, ID_MAX, id);
        }
        if id >= 5 {
            IdRange::High
        }
        else{
            IdRange::Low
        }
    }
}

// TODO there has to be a better way
// Dummy function to ensure these types are included in the C++ header
#[no_mangle]
pub extern "C" fn types_dummy(a : CmdMode, b : FbField, c : Feedback, d : Gm6020Can){
}

pub fn init(interface: &str) -> Result<Box<Gm6020Can>, String> {
    let mut gm6020_can: Box<Gm6020Can> = Box::new(Gm6020Can::default());                                  // Box is like std::unique_ptr in C++
    gm6020_can.as_mut().socket = Some(CanSocket::open(&interface).map_err(|err| err.to_string())?);       // Attempt to open the given interface
    let filter = CanFilter::new(FB_ID_BASE as u32, 0xffff - 0xf);                                         // Create a filter to only accept messages with IDs from 0x200 to 0x20F (Motor feedbacks are 0x205 to 0x20B)
    gm6020_can.as_ref().socket.as_ref().unwrap().set_filters(&[filter]).map_err(|err| err.to_string())?;  // Apply the filter to our interface
    // Read 100 frames to populate feedbacks - this prevents the run loop from thinking motors aren't initialized
    for _ in 1..100 {
        match gm6020_can.as_mut().socket.as_ref().unwrap().read_frame(){
            Ok(CanFrame::Data(frame)) => rx_fb(gm6020_can.as_mut(), frame),
            Ok(CanFrame::Remote(frame)) => eprintln!("{:?}", frame),
            Ok(CanFrame::Error(frame)) => eprintln!("{:?}", frame),
            Err(err) => eprintln!("{}", err),
        };
    }
    return Ok(gm6020_can);
}




pub fn run_once(gm6020_can: &mut Gm6020Can) -> Result<(), String>{
    // Read all available frames from buffer
    let mut timed_out: bool = false;
    while !timed_out {
        match gm6020_can.socket.as_ref().unwrap().read_frame_timeout(Duration::from_micros(1)){ // Feedbacks come in at 1kHz, much longer period than 1μs
            Ok(CanFrame::Data(frame)) => rx_fb(gm6020_can, frame),
            Ok(CanFrame::Remote(frame)) => eprintln!("{:?}", frame),
            Ok(CanFrame::Error(frame)) => eprintln!("{:?}", frame),
            Err(err) => if err.to_string() == "timed out" {timed_out=true} else {eprintln!("{}", err)},
        };
    }

    // If a motor is not Disabled did not report any feedback for 100ms, report an error
    for i in 0 .. gm6020_can.feedbacks.len() {
        if gm6020_can.modes[i] != CmdMode::Disabled && gm6020_can.feedbacks[i].0.ok_or_else(|| Err::<(), String>(format!("Motor {} never responded.", (i as u8)+ID_MIN))).unwrap().elapsed().map_err(|err| err.to_string())?.as_millis() >= 100 {
            eprintln!("Haven't heard from Motor {} in over 100ms. Are you reading frequently enough?", (i as u8)+ID_MIN);
        }
    }

    // Loop through all motors and check which combinations of IdRange and CmdMode actually need to be sent
    let mut i_l: bool = false;
    let mut i_h: bool = false;
    let mut v_l: bool = false;
    let mut v_h: bool = false;
    for i in 0 .. gm6020_can.modes.len() {
        match (gm6020_can.modes[i], IdRange::from_u8(i as u8 + ID_MIN)) {
            (CmdMode::Voltage, IdRange::Low ) => v_l = true,
            (CmdMode::Voltage, IdRange::High) => v_h = true,
            (CmdMode::Current, IdRange::Low ) => i_l = true,
            (CmdMode::Current, IdRange::High) => i_h = true,
            (_, _) => (),
        };
    }
    // Send the commands, accumulating the results to return
    let mut r: Result<(), String> = Ok(());
    if v_l {r = r.and_then(|_| tx_cmd(gm6020_can, IdRange::Low,  CmdMode::Voltage));}
    if v_h {r = r.and_then(|_| tx_cmd(gm6020_can, IdRange::High, CmdMode::Voltage));}
    if i_l {r = r.and_then(|_| tx_cmd(gm6020_can, IdRange::Low,  CmdMode::Current));}
    if i_h {r = r.and_then(|_| tx_cmd(gm6020_can, IdRange::High, CmdMode::Current));}
    return r;
}


pub fn set_cmd(gm6020_can: &mut Gm6020Can, id: u8, mode: CmdMode, cmd: f64) -> Result<(), String> {
    // Check id range and convert to array index
    if id<ID_MIN || id>ID_MAX { return Err(format!("id out of range [{}, {}]: {}", ID_MIN, ID_MAX, id)); }
    let idx = (id-1) as usize;
    // If the motor is too hot, write 0 command and return error
    if gm6020_can.feedbacks[idx].1.temperature >= TEMP_MAX as u16 { gm6020_can.commands[idx] = 0; return Err(format!("temperature overload [{}]: {}", TEMP_MAX, gm6020_can.feedbacks[idx].1.temperature));}
    // Convert torque and velocity commands to corresponding current and voltage commands
    if mode == CmdMode::Torque {return set_cmd(gm6020_can, id, CmdMode::Current, cmd/NM_PER_A);}
    if mode == CmdMode::Velocity  {return set_cmd(gm6020_can, id, CmdMode::Voltage, cmd*RPM_PER_ANGULAR/RPM_PER_V);}
    // Limit to max allowable command values
    if mode == CmdMode::Voltage && cmd.abs() > V_MAX {
        eprintln!("Warning: voltage out of range [{}, {}]: {}. Clamping.", -1.0*V_MAX, V_MAX, cmd);
        return set_cmd(gm6020_can, id, CmdMode::Voltage, V_MAX*cmd.abs()/cmd);
    }
    if mode == CmdMode::Current && cmd.abs() > I_MAX {
        eprintln!("Warning: current out of range [{}, {}]: {}. Clamping.", -1.0*I_MAX, I_MAX, cmd);
        return set_cmd(gm6020_can, id, CmdMode::Current, I_MAX*cmd.abs()/cmd);
    }
    // Change the motor's mode if necessary
    if gm6020_can.modes[idx] == CmdMode::Disabled {
        println!("Setting motor {} to mode {}", id, mode);
        gm6020_can.modes[idx] = mode;
    }
    // A motor's mode shouldn't change at runtime because it requires setting a parameter in RoboMaster Assistant
    else if gm6020_can.modes[idx] != mode {
        eprintln!("Warning! Changing mode of motor {} from {} to {}", id, gm6020_can.modes[idx], mode);
        gm6020_can.modes[idx] = mode;
    }

    // Map the volts/amps command to the low-level range expected by the motor
    gm6020_can.commands[idx] = match mode {
        CmdMode::Voltage => (V_CMD_MAX*cmd/V_MAX) as i16,
        CmdMode::Current => (I_CMD_MAX*cmd/I_MAX) as i16,
        _ => panic!(),
    };
    Ok(())
}

/*
**  Send a CAN frame with motor commands
**
**  gm6020_can: 'handle' to act upon
**  id_range: send to low [1,4] or high [5,7] motors
**  mode: send voltage or current commands
*/
fn tx_cmd(gm6020_can: &mut Gm6020Can, id_range: IdRange, mode: CmdMode) -> Result<(), String> {
    // Determine which CAN id to send based on the command mode and id range
    let id: u16 = match (mode, id_range) {
        (CmdMode::Voltage, IdRange::Low ) => CMD_ID_V_L,
        (CmdMode::Voltage, IdRange::High) => CMD_ID_V_H,
        (CmdMode::Current, IdRange::Low ) => CMD_ID_I_L,
        (CmdMode::Current, IdRange::High) => CMD_ID_I_H,
        (_, _) => panic!(),
    };

    // Slice half of the commands array, depending on the id range
    let cmds: &[i16] = &gm6020_can.commands[((id_range as u8) * 4) as usize .. (4 + (id_range as u8)*4) as usize];
    // Construct a CAN frame using the ID and cmds data
    let frame = CanFrame::new(
        StandardId::new(id).unwrap(),
        &[(cmds[0]>>8) as u8, cmds[0] as u8, (cmds[1]>>8) as u8, cmds[1] as u8, (cmds[2]>>8) as u8, cmds[2] as u8, (cmds[3]>>8) as u8, cmds[3] as u8])
        .ok_or_else(|| Err::<CanFrame, String>("Failed to open socket".to_string())).unwrap();
    // Write the frame
    gm6020_can.socket.as_ref().ok_or_else(|| Err::<CanSocket, String>("Socket not initialized".to_string())).unwrap().write_frame(&frame).map_err(|err| err.to_string())?;
    Ok(())
}


/*
**  Parse a received feedback frame
**
**  gm6020_can: the handle to update
**  frame: the CAN frame to parse
*/
fn rx_fb(gm6020_can: &mut Gm6020Can, frame: CanDataFrame){
    // Convert CAN frame ID to motor ID
    let rxid: u16 = frame.raw_id() as u16;
    let id: u8 = (rxid-FB_ID_BASE)as u8;
    if id<ID_MIN || id>ID_MAX {return;}

    // Get a reference to the feedback object and data array to simplify the parsing code
    let f: &mut (Option<SystemTime>, Feedback) = &mut gm6020_can.feedbacks[(id-1) as usize];
    let d: &[u8] = &frame.data()[0..8];
    // Pull the feedback values out of the data array and save them in the feedback object
    f.0 = Some(SystemTime::now());// TODO waiting on socketcan library to implement hardware timestamps
    f.1.position    = (d[0] as u16) << 8 | d[1] as u16;
    f.1.velocity    = (d[2] as i16) << 8 | d[3] as i16;
    f.1.current     = (d[4] as i16) << 8 | d[5] as i16;
    f.1.temperature = d[6] as u16;
}


pub fn get_state(gm6020_can: &mut Gm6020Can, id: u8, field: FbField) -> f64{
    match field {
        FbField::Position    => gm6020_can.feedbacks[(id-1)as usize].1.position as f64/POS_MAX as f64 *2f64*PI,
        FbField::Velocity    => gm6020_can.feedbacks[(id-1)as usize].1.velocity as f64/RPM_PER_ANGULAR,
        FbField::Current     => gm6020_can.feedbacks[(id-1)as usize].1.current as f64/1000f64/I_FB_MAX,
        FbField::Temperature => gm6020_can.feedbacks[(id-1)as usize].1.temperature as f64,
    }
}