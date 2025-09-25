use std::fmt;
use socketcan::{CanFilter, CanFrame, CanSocket, Frame, Socket, SocketOptions};
use std::f64::consts::PI;
use std::time::Duration;
use embedded_can::{Frame as EmbeddedFrame, StandardId};
use std::time::SystemTime;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;

const FB_ID_BASE_6020: u16 = 0x204;
const FB_ID_BASE_3508: u16 = 0x200;
const CMD_ID_V_L_6020: u16 = 0x1ff;
const CMD_ID_V_H_6020: u16 = 0x2ff;
const CMD_ID_I_L_6020: u16 = 0x1fe;
const CMD_ID_I_H_6020: u16 = 0x2fe;
const CMD_ID_I_L_3508: u16 = 0x200;
const CMD_ID_I_H_3508: u16 = CMD_ID_V_L_6020;
const CMD_ID_I_L_2006: u16 = CMD_ID_I_L_3508;
const CMD_ID_I_H_2006: u16 = CMD_ID_I_H_3508;


pub const ID_MIN: u8 = 1;
#[no_mangle]
pub extern "C" fn id_max(motor_type: MotorType) -> u8 {
    match motor_type {
        MotorType::GM6020 => 7,
        MotorType::M3508  => 8,
        MotorType::M2006  => 8,
    }
}
const POS_MAX   : u16 = 8191;
pub const RPM_PER_ANGULAR : f64 = 60.0/(2.0*3.14159);
pub const RPM_PER_V: f64 = 13.33; // GM6020 only

// Amps ("torque current")
#[no_mangle]
pub extern "C" fn i_max(motor_type: MotorType) -> f64 {
    match motor_type {
        MotorType::GM6020 => 1.62,
        MotorType::M3508  => 20.0,
        MotorType::M2006  => 10.0,
    }
}
#[no_mangle]
pub extern "C" fn nm_per_a(motor_type: MotorType) -> f64 {
    match motor_type {
        MotorType::GM6020 => 0.741,
        MotorType::M3508  => 0.353, // approximated from datasheet graph
        MotorType::M2006  => 0.338, // approximated from datasheet graph
    }
}
pub const V_MAX      : f64 =  24.0;  // Volts DC
pub const TEMP_MAX   : u8  = 125;    // C
const V_CMD_MAX: f64 = 25000.0;     // V_MAX maps to V_CMD_MAX in the CAN messages
// I_MAX maps to I_CMD_MAX in the CAN messages
#[no_mangle]
pub extern "C" fn i_cmd_max(motor_type: MotorType) -> f64 {
    match motor_type {
        MotorType::GM6020 => 16384.0,
        MotorType::M3508  => 16384.0,
        MotorType::M2006  => 10000.0,
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
#[repr(C)]
pub enum CmdMode { Disabled=-1, Voltage, Current, Torque, Velocity }
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
pub enum MotorType { GM6020, M3508, M2006}
impl Default for MotorType {
    fn default() -> Self { MotorType::GM6020 }
}
impl fmt::Display for MotorType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MotorType::GM6020 => write!(f, "GM6020"),
            MotorType::M3508  => write!(f, "M3508"),
            MotorType::M2006  => write!(f, "M2006"),

        }
    }
}


#[derive(Copy, Clone, PartialEq, Eq)]
#[repr(C)]
pub enum FbField { Position, Velocity, Current, Temperature }
impl Default for FbField {
    fn default() -> Self { FbField::Position }
}
impl fmt::Display for FbField {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FbField::Position     => write!(f, "Position"),
            FbField::Velocity     => write!(f, "Velocity"),
            FbField::Current      => write!(f, "Current"),
            FbField::Temperature  => write!(f, "Temperature"),
        }
    }
}

#[derive(Default, Debug)]
struct Feedback {
    position:    u16, // [0, 8191]
    velocity:    i16, // rpm
    current:     i16, // [-16384, 16384]:[-1.62A, 1.62A]
    temperature: u16, // C
}

// Technically we could handle more than 8 motors at once since the M3508 and GM6020 ID ranges only
// partially overlap. However, that would greatly complicate things and it is a rare use case.
const ARR_LEN: usize = 8;
#[derive(Default)]
#[repr(C)]
pub struct RmMotorsCan {
    socket: Mutex<Option<CanSocket>>,
    motor_types : RwLock<[MotorType; ARR_LEN]>,
    modes       : RwLock<[CmdMode; ARR_LEN]>,
    commands    : RwLock<[i16; ARR_LEN]>,
    feedbacks   : RwLock<[(Option<SystemTime>, Feedback); ARR_LEN]>,
    upper_3508  : RwLock<bool>, // if true, parse CAN ID range 0x205-0x208 as m3508/m2006
}

#[derive(Copy, Clone, Debug)]
enum IdRange { Low, High }
impl IdRange {
    fn from_u8(id: u8) -> IdRange {
        if id < ID_MIN || id > ARR_LEN as u8 {
            panic!("id out of range [{}, {}]: {}", ID_MIN, ARR_LEN, id);
        }
        if id >= 5 {
            IdRange::High
        }
        else{
            IdRange::Low
        }
    }
}


pub fn init_bus(interface: &str) -> Result<Arc<RmMotorsCan>, String> {
    let rm_motors_can: Arc<RmMotorsCan> = Arc::new(RmMotorsCan::default());                      // Arc (Atomically Reference Counted) is like shared_ptr in C++
    let socket: CanSocket = CanSocket::open(&interface).map_err(|err| err.to_string())?;  // Attempt to open the given interface

    // Listen for 100ms to check if a CAN bus driver is already running- don't want to send conflicting commands.
    let t: SystemTime = SystemTime::now();
    while t.elapsed().map_err(|err| err.to_string())?.as_millis() < 100 {
        match socket.read_frame(){
            Err(err) => {return Err(err.to_string())},
            Ok(CanFrame::Remote(_)) => (),
            Ok(CanFrame::Error(_)) => (),
            Ok(CanFrame::Data(frame)) => {
                let frame_id: u16 = frame.raw_id() as u16;
                if frame_id == CMD_ID_V_L_6020 || frame_id == CMD_ID_V_H_6020 || frame_id == CMD_ID_I_L_6020 || frame_id == CMD_ID_I_H_6020 || frame_id == CMD_ID_I_L_3508 || frame_id == CMD_ID_I_H_3508 {
                    return Err(String::from("Another program is sending GM6020 commands already"));
                }
            },
        };
    }

    let filter: CanFilter = CanFilter::new(FB_ID_BASE_3508 as u32, 0xffff - 0xf);  // Create a filter to only accept messages with IDs from 0x200 to 0x20F (Motor feedbacks are 0x201 to 0x20B)
    socket.set_filters(&[filter]).map_err(|err| err.to_string())?;            // Apply the filter to our interface
    *rm_motors_can.socket.lock().unwrap() = Some(socket);                        // Attach the socket to the rm_motors_can object for future reading and writing

    // Read frames to populate feedbacks - this prevents run_once from thinking motors aren't initialized
    thread::sleep(std::time::Duration::from_millis(5));
    rx_fb(rm_motors_can.clone())?;

    return Ok(rm_motors_can);
}

pub fn init_motor(rm_motors_can: Arc<RmMotorsCan>, id: u8, motor_type: MotorType, mode: CmdMode) -> Result<i32, String> {
    if (motor_type == MotorType::M3508 || motor_type == MotorType::M2006) && (mode == CmdMode::Voltage || mode == CmdMode::Velocity){
        return Err(format!("Attempting to initialize motor {} in {} mode, but it is an {} which only accepts Current or Torque commands", id, mode, motor_type));
    }
    let idx: usize = (id-1) as usize;

    // Check for ID collisions - this is a limitation of DJI's address scheme
    if motor_type == MotorType::GM6020 && id < 5 {
        for i in 4 .. 8 {
            if rm_motors_can.modes.read().unwrap()[i] == CmdMode::Disabled {continue;}
            let i_type : MotorType = rm_motors_can.motor_types.read().unwrap()[i];
            if i_type == MotorType::M3508 || i_type == MotorType::M2006 {
                return Err(format!("GM6020 ID 1-4 cannot coexist with {} ID 5-8", i_type));
            }
        }
    }
    else if (motor_type == MotorType::M3508 || motor_type == MotorType::M2006) && id > 4 {
        for i in 0 .. 5 {
            if rm_motors_can.modes.read().unwrap()[i] == CmdMode::Disabled {continue;}
            if rm_motors_can.motor_types.read().unwrap()[i] == MotorType::GM6020 {
                return Err(format!("{} ID 5-8 cannot coexist with GM6020 ID 1-4", motor_type));
            }
        }
        // Set the flag to indicate we will be parsing CAN ID range 0x205-0x208 as m3508/m2006
        *rm_motors_can.upper_3508.write().unwrap() = true;
    }

    let type_actual: MotorType = rm_motors_can.motor_types.read().unwrap()[idx];
    let mode_actual: CmdMode = rm_motors_can.modes.read().unwrap()[idx];
    if mode_actual == CmdMode::Disabled {
        println!("Initializing {}:{} in {} mode", motor_type, id, mode);
    }
    // A motor's mode shouldn't change at runtime because it requires setting a parameter in RoboMaster Assistant
    else{
        if mode_actual != mode {
            eprintln!("Warning: Changing {}:{} from {} to {} mode", motor_type, id, mode_actual, mode);
        }
        if type_actual != motor_type {
            eprintln!("Warning: Changing motor {} from {} to {} type", id, type_actual, motor_type);
        }
    }
    rm_motors_can.motor_types.write().unwrap()[idx] = motor_type;
    rm_motors_can.modes.write().unwrap()[idx] = mode;
    return Ok(0);
}

/*
**  Clean up pointers and release the socket
**  rm_motors_can: 'handle' to act upon
**  Doesn't really need to return anything but this is convenient for macros since all the functions return a result.
*/
pub fn cleanup(rm_motors_can: Arc<RmMotorsCan>, period_ms: u64) -> Result<i32, String> {
    // Ramp down commands to avoid jerking stop
    let mut threads: Vec<thread::JoinHandle<()>> = Vec::new();
    for i in 0 .. ARR_LEN {
        let m: CmdMode = rm_motors_can.modes.read().unwrap()[i];
        if m == CmdMode::Disabled {continue;}
        let gmc: Arc<RmMotorsCan> = rm_motors_can.clone();
        // Multi-thread so all motors spin down at once
        threads.push(thread::spawn( move ||{
            let motor_type: MotorType = gmc.motor_types.read().unwrap()[i];

            // Avoid get_state - simplify by using only voltage and current commands
            let mut cmd: f64 = match m {
                CmdMode::Voltage  => gmc.commands.read().unwrap()[i] as f64 /V_CMD_MAX*V_MAX,
                CmdMode::Velocity => gmc.commands.read().unwrap()[i] as f64 /V_CMD_MAX*V_MAX,
                CmdMode::Current  => gmc.commands.read().unwrap()[i] as f64 /i_cmd_max(motor_type)*i_max(motor_type),
                CmdMode::Torque   => gmc.commands.read().unwrap()[i] as f64 /i_cmd_max(motor_type)*i_max(motor_type),
                CmdMode::Disabled => panic!("Attempting to clean up disabled motor"),
            };
            let sign: f64 = cmd/cmd.abs();
            loop{
                if cmd.abs() <= 0.2 || period_ms == 0 {
                    set_cmd(gmc.clone(), i as u8+ID_MIN, 0f64).map_or_else(|e| eprintln!("{}", e), |_| ());
                    run_once(gmc.clone()).map_or_else(|e| eprintln!("{}", e), |_| ());
                    break;
                }
                cmd = cmd-sign*0.2;
                set_cmd(gmc.clone(), i as u8+ID_MIN, cmd).map_or_else(|e| eprintln!("{}", e), |_| ());
                run_once(gmc.clone()).map_or_else(|e| eprintln!("{}", e), |_| ());
                thread::sleep(std::time::Duration::from_millis(period_ms));                
            }
        }));
    }
    for thread in threads.into_iter() {
        thread.join().expect("Couldn't join the cleanup thread");
    }
    Ok(0)
}




pub fn run_once(rm_motors_can: Arc<RmMotorsCan>) -> Result<i32, String>{
    rx_fb(rm_motors_can.clone())?;

    // Loop through all motors and check which combinations of IdRange and CmdMode actually need to be sent
    let mut flags: [bool; ARR_LEN+1] = [false; ARR_LEN+1];
    for i in 0 .. ARR_LEN {
        let mode: CmdMode = match rm_motors_can.modes.read().unwrap()[i] {
            CmdMode::Current  => CmdMode::Current,
            CmdMode::Torque   => CmdMode::Current,
            CmdMode::Voltage  => CmdMode::Voltage,
            CmdMode::Velocity => CmdMode::Voltage,
            CmdMode::Disabled => CmdMode::Disabled,
        };
        if mode == CmdMode::Disabled {continue;}
        flags[match (mode, IdRange::from_u8(i as u8 + ID_MIN), rm_motors_can.motor_types.read().unwrap()[i]) {
            (CmdMode::Voltage, IdRange::Low , MotorType::GM6020) => 0,
            (CmdMode::Voltage, IdRange::High, MotorType::GM6020) => 1,
            (CmdMode::Current, IdRange::Low , MotorType::GM6020) => 2,
            (CmdMode::Current, IdRange::High, MotorType::GM6020) => 3,
            (CmdMode::Current, IdRange::Low , MotorType::M3508 ) => 4,
            (CmdMode::Current, IdRange::Low , MotorType::M2006 ) => 5,
            (CmdMode::Current, IdRange::High, MotorType::M3508 ) => 6,
            (CmdMode::Current, IdRange::High, MotorType::M2006 ) => 7,
            (_, _, _) => 8,
        }] = true;
    }
    // Send the commands, accumulating the results to return
    let mut r: Result<i32, String> = Ok(0);
    for i in 0 .. ARR_LEN {
        if flags[i] {
            r = r.and_then(
                |_| match i {
                    0 => tx_cmd(rm_motors_can.clone(), CMD_ID_V_L_6020, IdRange::Low),
                    1 => tx_cmd(rm_motors_can.clone(), CMD_ID_V_H_6020, IdRange::High),
                    2 => tx_cmd(rm_motors_can.clone(), CMD_ID_I_L_6020, IdRange::Low),
                    3 => tx_cmd(rm_motors_can.clone(), CMD_ID_I_H_6020, IdRange::High),
                    4 => tx_cmd(rm_motors_can.clone(), CMD_ID_I_L_3508, IdRange::Low),
                    5 => tx_cmd(rm_motors_can.clone(), CMD_ID_I_L_2006, IdRange::Low),
                    6 => tx_cmd(rm_motors_can.clone(), CMD_ID_I_H_3508, IdRange::High),
                    7 => tx_cmd(rm_motors_can.clone(), CMD_ID_I_H_2006, IdRange::High),
                    _ => return Err(format!("Unknown combination of CmdMode, IdRange, MotorType in run_once")),
                }
            );
        }
    }
    return r;
}


pub fn set_cmd(rm_motors_can: Arc<RmMotorsCan>, id: u8, cmd: f64) -> Result<i32, String> {
    // convert ID to array index
    let idx: usize = (id-1) as usize;
    // Check id range
    if id<ID_MIN || id>id_max(rm_motors_can.motor_types.read().unwrap()[idx]) { return Err(format!("id out of range [{}, {}]: {}", ID_MIN, id_max(rm_motors_can.motor_types.read().unwrap()[idx]), id)); }
    // If the motor is too hot, write 0 command and return error
    // TODO what to do about m3508, m2006?
    if rm_motors_can.feedbacks.read().unwrap()[idx].1.temperature >= TEMP_MAX as u16 { rm_motors_can.commands.write().unwrap()[idx] = 0; return Err(format!("temperature overload [{}]: {}", TEMP_MAX, rm_motors_can.feedbacks.read().unwrap()[idx].1.temperature));}
    let mut mode: CmdMode = rm_motors_can.modes.read().unwrap()[idx];
    let motor_type: MotorType = rm_motors_can.motor_types.read().unwrap()[idx];
    let mut cmd_actual: f64 = cmd;
    // Convert torque and velocity commands to corresponding current and voltage commands
    if mode == CmdMode::Torque {
        mode = CmdMode::Current;
        cmd_actual/=nm_per_a(motor_type);
    }
    if mode == CmdMode::Velocity {
        mode = CmdMode::Voltage;
        cmd_actual*=RPM_PER_ANGULAR/RPM_PER_V;
    }
    // Limit to max allowable command values
    if mode == CmdMode::Voltage && cmd_actual.abs() > V_MAX {
        eprintln!("Warning: voltage out of range [{}, {}]: {}. Clamping.", -1.0*V_MAX, V_MAX, cmd);
        cmd_actual = V_MAX*cmd.abs()/cmd;
    }
    let i_max: f64 = i_max(motor_type);

    if mode == CmdMode::Current && cmd_actual.abs() > i_max {
        eprintln!("Warning: current out of range [{}, {}]: {}. Clamping.", -1.0*i_max, i_max, cmd);
        cmd_actual = i_max*cmd.abs()/cmd;
    }

    rm_motors_can.commands.write().unwrap()[idx] = match mode {
        CmdMode::Voltage => (V_CMD_MAX*cmd_actual/V_MAX) as i16,
        CmdMode::Current => (i_cmd_max(motor_type)*cmd_actual/i_max) as i16,
        _ => panic!("Invalid mode, logic error in `set_cmd`"),
    };
    Ok(0)
}

/*
**  Send a CAN frame with motor commands
**
**  rm_motors_can: 'handle' to act upon
**  id_range: send to low [1,4] or high [5,7] motors
**  mode: send voltage or current commands
*/
fn tx_cmd(rm_motors_can: Arc<RmMotorsCan>, frame_id: u16, id_range: IdRange) -> Result<i32, String> {
    // Slice half of the commands array, depending on the id range
    let cmds: &[i16] = &rm_motors_can.commands.read().unwrap()[((id_range as u8) * 4) as usize .. (4 + (id_range as u8)*4) as usize];
    // Construct a CAN frame using the ID and cmds data
    let frame = CanFrame::new(
        StandardId::new(frame_id).unwrap(),
        &[(cmds[0]>>8) as u8, cmds[0] as u8, (cmds[1]>>8) as u8, cmds[1] as u8, (cmds[2]>>8) as u8, cmds[2] as u8, (cmds[3]>>8) as u8, cmds[3] as u8])
        .ok_or_else(|| Err::<CanFrame, String>("Failed to open socket".to_string())).unwrap();
    // Write the frame
    rm_motors_can.socket.lock().unwrap().as_ref().ok_or_else(|| Err::<CanSocket, String>("Socket not initialized".to_string())).unwrap().write_frame(&frame).map_err(|err| err.to_string())?;
    Ok(0)
}


/*
**  Parse a received feedback frame
**
**  rm_motors_can: the handle to update
**  frame: the CAN frame to parse
*/
fn rx_fb(rm_motors_can: Arc<RmMotorsCan>) -> Result<i32, String> {
    // If a motor is not Disabled did not report any feedback for 100ms, report an error
    for i in 0 .. ARR_LEN {
        if rm_motors_can.modes.read().unwrap()[i] != CmdMode::Disabled && rm_motors_can.feedbacks.read().unwrap()[i].0.ok_or_else(|| format!("Motor {} never responded.", (i as u8)+ID_MIN))?.elapsed().map_err(|err| err.to_string())?.as_millis() >= 100 {
            eprintln!("Haven't heard from Motor {} in over 100ms. Are you reading frequently enough?", (i as u8)+ID_MIN);
        }
    }

    // Read all available frames from buffer
    let mut timed_out: bool = false;
    while !timed_out {
        // Keep timeout very short because we don't want to wait for new frames to arrive
        match rm_motors_can.socket.lock().unwrap().as_ref().ok_or_else(|| Err::<CanSocket, String>("Socket not initialized".to_string())).unwrap().read_frame_timeout(Duration::from_micros(1)){
            Err(err) => if err.to_string() == "timed out" {timed_out=true} else {return Err(err.to_string())}
            Ok(CanFrame::Remote(_)) => (), // The mask on the socket isn't a perfect match, so it's possible we receive a remote frame for another device with a nearby id
            Ok(CanFrame::Error(frame)) => eprintln!("{:?}", frame), // The datasheet didn't mention any error frames but we might as well print them
            Ok(CanFrame::Data(frame)) => {
                // Convert CAN frame ID to motor ID
                let rxid: u16 = frame.raw_id() as u16;
                let id: u8;
                // M3508 ID range
                if rxid <= 0x204 || (rxid > 0x204 && rxid <= 0x208 && *rm_motors_can.upper_3508.read().unwrap()) {
                    id = (rxid-FB_ID_BASE_3508) as u8;
                }
                // MG6020 ID range
                else if rxid > 0x204 && rxid <= 0x20B {
                    id = (rxid-FB_ID_BASE_6020) as u8;
                }
                else {
                    continue;
                }

                // Get a reference to the feedback object and data array to simplify the parsing code
                let f: &mut (Option<SystemTime>, Feedback) = &mut rm_motors_can.feedbacks.write().unwrap()[(id-1) as usize];
                let d: &[u8] = &frame.data()[0..ARR_LEN];
                // Pull the feedback values out of the data array and save them in the feedback object
                f.0 = Some(SystemTime::now());// TODO waiting on socketcan library to implement hardware timestamps
                f.1.position    = (d[0] as u16) << 8 | d[1] as u16;
                f.1.velocity    = (d[2] as i16) << 8 | d[3] as i16;
                f.1.current     = (d[4] as i16) << 8 | d[5] as i16;
                f.1.temperature = d[6] as u16;
            },
        };
    }
    Ok(0)
}


pub fn get_state(rm_motors_can: Arc<RmMotorsCan>, id: u8, field: FbField) -> Result<f64, String>{
    let motor_type: MotorType = rm_motors_can.motor_types.read().unwrap()[(id-1) as usize];
    if motor_type == MotorType::M2006 && (field == FbField::Current || field == FbField::Temperature){
        return Err(format!("Motor {} is an M2006, which does not report {}", id, field));
    }
    Ok(match field {
        FbField::Position    => rm_motors_can.feedbacks.read().unwrap()[(id-1)as usize].1.position as f64/POS_MAX as f64 *2f64*PI,
        FbField::Velocity    => rm_motors_can.feedbacks.read().unwrap()[(id-1)as usize].1.velocity as f64/RPM_PER_ANGULAR,
        FbField::Current     => rm_motors_can.feedbacks.read().unwrap()[(id-1)as usize].1.current as f64/i_cmd_max(motor_type)*i_max(motor_type),
        FbField::Temperature => rm_motors_can.feedbacks.read().unwrap()[(id-1)as usize].1.temperature as f64,
    })
}
