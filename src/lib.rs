use std::f64::consts::PI;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use embedded_can::{Frame as EmbeddedFrame, StandardId};
use socketcan::{CanDataFrame, CanFilter, CanFrame, CanSocket, Frame, Socket, SocketOptions};
use std::ptr::null;
use std::time::SystemTime;
use std::ffi::CStr;
use std::os::raw::c_char;
use std::thread;
use std::sync::Arc;


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

#[derive(Copy, Clone, Debug)]
#[repr(C)]
enum IdRange { Low, High }
impl IdRange {
    fn from_u8(id: u8) -> IdRange {
        if id < ID_MIN || id > ID_MAX {
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


// TODO handle CAN no buffer left
// TODO try to reduce panics and handle errors more gracefully

/*
**  interface: SocketCAN interface name e.g. "can0"
**  returns: pointer to Gm6020Can struct, to be passed to other functions in this library
*/
#[no_mangle]
pub extern "C" fn gm6020_can_init(interface: *const c_char) -> *mut Gm6020Can {
    let inter: &str;
    if interface.is_null() {
        println!("Invalid c-string received for interface name (null pointer)");
        return null::<Gm6020Can>() as *mut Gm6020Can;
    }
    else {
        unsafe {
            let r = CStr::from_ptr(interface).to_str();
            if r.is_err() {
                eprintln!("Invalid c-string received for interface name");
                return null::<Gm6020Can>() as *mut Gm6020Can;
            }
            inter = r.unwrap();
        }
    }
    _init(inter).map_or_else(|e| {eprintln!("{}", e); null::<Gm6020Can>() as *mut Gm6020Can}, |v| Box::into_raw(v) as *mut Gm6020Can)
}
fn _init(interface: &str) -> Result<Box<Gm6020Can>, String> {
    let mut gm6020_can: Box<Gm6020Can> = Box::new(Gm6020Can::default());                                  // Box is like std::unique_ptr in C++
    gm6020_can.as_mut().socket = Some(CanSocket::open(&interface).map_err(|err| err.to_string())?);       // Attempt to open the given interface
    let filter = CanFilter::new(FB_ID_BASE as u32, 0xffff - 0xf);                                         // Create a filter to only accept messages with IDs from 0x200 to 0x20F (Motor feedbacks are 0x205 to 0x20B)
    gm6020_can.as_ref().socket.as_ref().unwrap().set_filters(&[filter]).map_err(|err| err.to_string())?;  // Apply the filter to our interface
    return Ok(gm6020_can);
}


/*
**  Clean up pointers and release the socket
**  gm6020_can: 'handle' to act upon
**  run_stopper: raw pointer to Arc<AtomicBool> which is shared with the run thread
*/
#[no_mangle]
pub extern "C" fn gm6020_can_cleanup(gm6020_can: *mut Gm6020Can, run_stopper: *mut std::ffi::c_void){
    if gm6020_can.is_null(){
        eprintln!("Invalid handle (null pointer)");
    }
    else{
        unsafe{drop(Box::from_raw(gm6020_can));}// Delete the pointer. The socket is automatically closed when the object is dropped.
    }
    if run_stopper.is_null() {
        eprintln!("Invalid run_stopper (null pointer)");
    }
    else{
        let stop: &mut Arc<AtomicBool> = unsafe{&mut *(run_stopper as *mut Arc<AtomicBool>)};
        stop.store(true, Ordering::Relaxed); // Stop the thread
        unsafe{drop(Box::from_raw(run_stopper))}; // Delete the pointer - actually just decreases the reference count, it won't be deleted until the thread also drops its reference.
    }
}


// TODO Below 100Hz the feedback values get weird - CAN buffer filling up?
/*
**  Spawn a thread to continuously update motor feedbacks and send commands.
**
**  gm6020_can: 'handle' to act upon
**  period_ms: run loop period in milliseconds
**  returns: 0 on success, -1 otherwise
*/
#[no_mangle]
pub extern "C" fn gm6020_can_run(gm6020_can: *mut Gm6020Can, period_ms: u64) -> *mut std::ffi::c_void{
    let handle: &mut Gm6020Can;
    if gm6020_can.is_null(){
        println!("Invalid handle (null pointer)");
        return null::<Arc<AtomicBool>>() as *mut std::ffi::c_void;
    }
    handle = unsafe{&mut *gm6020_can}; // Wrap the raw pointer into Rust object
    let shared_stop: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    let stop_ref_2: Arc<AtomicBool> = shared_stop.clone();
    thread::spawn( move ||
    while ! stop_ref_2.load(Ordering::Relaxed) {
        _run_once(handle).map_or_else(|e| eprintln!("{}", e), |_| ());
        thread::sleep(std::time::Duration::from_millis(period_ms));
    });

    return Box::into_raw(Box::new(shared_stop)) as *mut std::ffi::c_void;
}

/*
** Update motor feedbacks and send commands.
**
**  gm6020_can: 'handle' to act upon
**  returns: 0 on success, -1 otherwise
*/
#[no_mangle]
pub extern "C" fn gm6020_can_run_once(gm6020_can: *mut Gm6020Can) -> i8{
    let handle: &mut Gm6020Can;
    if gm6020_can.is_null(){
        println!("Invalid handle (null pointer)");
        return -1;
    }
    handle = unsafe{&mut *gm6020_can}; // Wrap the raw pointer into Rust object
    _run_once(handle).map_or_else(|e| {eprintln!("{}", e); -1_i8}, |_| 0_i8)
}
fn _run_once(gm6020_can: &mut Gm6020Can) -> Result<(), String>{
    // TODO instead should we read every frame available in the buffer?
    // Read one frame and parse the feedback values
    match gm6020_can.socket.as_ref().unwrap().read_frame_timeout(Duration::from_millis(2)) { // feedbacks sent at 1kHz, use 2ms for slight leeway
        Ok(CanFrame::Data(frame)) => rx_fb(gm6020_can, frame),
        Ok(CanFrame::Remote(frame)) => eprintln!("{:?}", frame),
        Ok(CanFrame::Error(frame)) => eprintln!("{:?}", frame),
        Err(err) => eprintln!("{}", err),
    };

    // If a motor is not Disabled did not report any feedback for 100ms, report an error
    for (i, fb) in (&gm6020_can.feedbacks).iter().enumerate() {
        if gm6020_can.modes[i] != CmdMode::Disabled && fb.0.ok_or_else(|| Err::<(), String>(format!("Motor {} never responded.", (i as u8)+ID_MIN))).unwrap().elapsed().map_err(|err| err.to_string())?.as_millis() >= 100 {
            eprintln!("Motor {} not responding. Are you reading frequently enough?", (i as u8)+ID_MIN);
        }
    }

    // Loop through all motors and check which combinations of IdRange and CmdMode actually need to be sent
    let mut i_l: bool = false;
    let mut i_h: bool = false;
    let mut v_l: bool = false;
    let mut v_h: bool = false;
    for (i, mode) in (&gm6020_can.modes).iter().enumerate() {
        match (mode, IdRange::from_u8(i as u8 + ID_MIN)) {
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


/*
** Convert various motor command types into the expected low-level format
**
**  gm6020_can: 'handle' to act upon
**  id: motor ID
**  mode: what type of command is being written
**  cmd: actual command value
*/
#[no_mangle]
pub extern "C" fn gm6020_can_set_cmd(gm6020_can: *mut Gm6020Can, id: u8, mode: CmdMode, cmd: f64) -> i8{
    let handle: &mut Gm6020Can;
    if gm6020_can.is_null(){
        println!("Invalid handle (null pointer)");
        return -1;
    }
    handle = unsafe{&mut *gm6020_can}; // Wrap the raw pointer into Rust object
    _set_cmd(handle, id, mode, cmd).map_or_else(|e| {eprintln!("{}", e); -1_i8}, |_| 0_i8)
}
fn _set_cmd(gm6020_can: &mut Gm6020Can, id: u8, mode: CmdMode, cmd: f64) -> Result<(), String> {
    // Check id range and convert to array index
    if id<ID_MIN || id>ID_MAX { return Err(format!("id out of range [{}, {}]: {}", ID_MIN, ID_MAX, id)); }
    let idx = (id-1) as usize;
    // If the motor is too hot, write 0 command and return error
    if gm6020_can.feedbacks[idx].1.temperature >= TEMP_MAX as u16 { gm6020_can.commands[idx] = 0; return Err(format!("temperature overload [{}]: {}", TEMP_MAX, gm6020_can.feedbacks[idx].1.temperature));}
    // Convert torque and velocity commands to corresponding current and voltage commands
    if mode == CmdMode::Torque {return _set_cmd(gm6020_can, id, CmdMode::Current, cmd/NM_PER_A);}
    if mode == CmdMode::Velocity  {return _set_cmd(gm6020_can, id, CmdMode::Voltage, cmd*RPM_PER_ANGULAR/RPM_PER_V);}
    // Limit to max allowable command values
    if mode == CmdMode::Voltage && cmd.abs() > V_MAX {
        eprintln!("Warning: voltage out of range [{}, {}]: {}. Clamping.", -1.0*V_MAX, V_MAX, cmd);
        return _set_cmd(gm6020_can, id, CmdMode::Voltage, V_MAX*cmd.abs()/cmd);
    }
    if mode == CmdMode::Current && cmd.abs() > I_MAX {
        eprintln!("Warning: current out of range [{}, {}]: {}. Clamping.", -1.0*I_MAX, I_MAX, cmd);
        return _set_cmd(gm6020_can, id, CmdMode::Current, I_MAX*cmd.abs()/cmd);
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

/*
**  Convert a motor's low-level feedback into normal units
**
**  gm6020_can: 'handle' to act upon
**  id: motor ID
**  field: the feedback item to get
*/
#[no_mangle]
pub extern "C" fn gm6020_can_get_state(gm6020_can: *mut Gm6020Can, id: u8, field: FbField) -> f64{
    if id<ID_MIN || id>ID_MAX { eprintln!("id out of range [{}, {}]: {}", ID_MIN, ID_MAX, id); panic!();}
    let handle: &mut Gm6020Can;
    if gm6020_can.is_null(){
        println!("Invalid handle");
        panic!();
    }
    handle = unsafe{&mut *gm6020_can}; // Wrap the raw pointer into Rust object
    match field {
        FbField::Position    => handle.feedbacks[(id-1)as usize].1.position as f64/POS_MAX as f64 *2f64*PI,
        FbField::Velocity    => handle.feedbacks[(id-1)as usize].1.velocity as f64/RPM_PER_ANGULAR,
        FbField::Current     => handle.feedbacks[(id-1)as usize].1.current as f64/I_CMD_MAX, // TODO units?
        FbField::Temperature => handle.feedbacks[(id-1)as usize].1.temperature as f64,
    }
}
