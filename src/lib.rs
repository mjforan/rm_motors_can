use std::time::Duration;
use embedded_can::{Frame as EmbeddedFrame, StandardId};
use socketcan::{CanDataFrame, CanFilter, CanFrame, CanSocket, Frame, Socket, SocketOptions};
use core::slice;
use std::ptr::null;
use std::time::SystemTime;
use std::ffi::CStr;
use std::os::raw::c_char;
use std::thread;


const FB_ID_BASE: u16 = 0x204;
const CMD_ID_V_L: u16 = 0x1ff;
const CMD_ID_V_H: u16 = 0x2ff;
const CMD_ID_I_L: u16 = 0x1fe;
const CMD_ID_I_H: u16 = 0x2fe;
const ID_MIN: u8 = 1;
const ID_MAX: u8 = 7;

const RPM_PER_V  : f64 =  13.33;
const N_PER_A    : f64 = 741.0;
const V_MAX      : f64 =  24.0;
const I_MAX      : f64 =   1.62;
const V_CMD_MAX : f64 = 25000.0;
const I_CMD_MAX : f64 = 16384.0;
const TEMP_MAX   : u8  = 125; // C

#[derive(Copy, Clone, PartialEq, Eq)]
#[repr(C)]
pub enum CmdMode { Voltage, Current, Torque, Speed }
#[derive(Copy, Clone)]
#[repr(C)]
enum IdRange { Low, High }
#[derive(Default)]
#[repr(C)]

struct Feedback {
    position:    u16, // [0, 8191]
    speed:       i16, // rpm
    current:     i16, // [-16384, 16384]:[-3A, 3A]
    temperature: u16,  // TODO units
}

#[derive(Default)]
#[repr(C)]
pub struct Gm6020Can {
    socket: Option<CanSocket>,
    feedbacks: [(Option<SystemTime>, Feedback); (ID_MAX-ID_MIN+1) as usize],
    commands: [i16; 8], // only 7 slots will be used but 8 is convenient for tx_cmd
}


// TODO split implementation and C wrapper into separate files
// TODO need rwlock on gm6020can.feedbacks

#[no_mangle]
pub extern "C" fn init(interface: *const c_char) -> *mut Gm6020Can{
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
    let mut gm6020_can: Box<Gm6020Can> = Box::new(Gm6020Can::default()); // TODO technically a memory leak - also make sure socket is closed
    gm6020_can.as_mut().socket = Some(CanSocket::open(&interface).map_err(|err| err.to_string())?);
    let filter = CanFilter::new(FB_ID_BASE as u32, 0xffffffff - 0xf);
    gm6020_can.as_ref().socket.as_ref().unwrap().set_filters(&[filter]).map_err(|err| err.to_string())?;
    return Ok(gm6020_can);
}

#[no_mangle]
pub extern "C" fn run(gm6020_can: *mut Gm6020Can, period: u64) -> i8{
    let handle: &mut Gm6020Can;
    if gm6020_can.is_null(){
        println!("Invalid handle (null pointer)");
        return -1;
    }
    else{
        handle = unsafe{&mut *gm6020_can};
    }
    thread::spawn( move || _run(handle, period).map_or_else(|e| {eprintln!("{}", e); -1_i8}, |_| 0_i8));
    return 0;
}
fn _run(gm6020_can: &mut Gm6020Can, period: u64) -> Result<(), String>{
    loop {
        _run_once(gm6020_can)?;
        thread::sleep(std::time::Duration::from_secs(period));
    }
}
#[no_mangle]
pub extern "C" fn run_once(gm6020_can: *mut Gm6020Can) -> i8{
    let handle: &mut Gm6020Can;
    if gm6020_can.is_null(){
        println!("Invalid handle (null pointer)");
        return -1;
    }
    else{
        handle = unsafe{&mut *gm6020_can};
    }
    _run_once(handle).map_or_else(|e| {eprintln!("{}", e); -1_i8}, |_| 0_i8)
}
fn _run_once(gm6020_can: &mut Gm6020Can) -> Result<(), String>{
    match gm6020_can.socket.as_ref().unwrap().read_frame_timeout(Duration::from_millis(10)) {
        Ok(CanFrame::Data(frame)) => rx_fb(gm6020_can, frame),
        Ok(CanFrame::Remote(frame)) => println!("CanRemoteFrame: {:?}", frame),
        Ok(CanFrame::Error(frame)) => println!("CanErrorFrame: {:?}", frame),
        Err(err) => eprintln!("{}", err),
    };

    // TODO check which ones actually need to be sent
/*
    let mut send_low: bool = false;
    let mut send_high: bool = false;
    for cmd in cmds.into_iter(){
        send_low |= cmd.0<=4;    
        send_high |= cmd.0>4;
        set_cmd(gm6020_can, cmd.0, mode, cmd.1)?;
    }

    match (send_low, send_high) {
        (false, false) => Err("Not sending any command".to_string()),
        (true,  false) => tx_cmd(gm6020_can, IdRange::Low, mode),
        (false, true)  => tx_cmd(gm6020_can, IdRange::High, mode),
        (true,  true)  => tx_cmd(gm6020_can, IdRange::Low, mode).and_then(|_| tx_cmd(gm6020_can, IdRange::Low, mode))
    }

    */
    for mode in [CmdMode::Voltage, CmdMode::Current]{
        for range in [IdRange::Low, IdRange::High]{
            tx_cmd(gm6020_can, range, mode)?;
        }
    }
    Ok(())
}

fn set_cmd(gm6020_can: &mut Gm6020Can, id: u8, mode: CmdMode, cmd: f64) -> Result<(), String> {
    let idx = (id-1) as usize;
    if gm6020_can.feedbacks[idx].1.temperature >= TEMP_MAX as u16 { gm6020_can.commands[idx] = 0; return Err(format!("temperature overload [{}]: {}", TEMP_MAX, gm6020_can.feedbacks[idx].1.temperature));}
    if mode == CmdMode::Torque {return set_cmd(gm6020_can, id, CmdMode::Current, cmd/N_PER_A);}
    if mode == CmdMode::Speed  {return set_cmd(gm6020_can, id, CmdMode::Voltage, cmd/RPM_PER_V);}
    if mode == CmdMode::Voltage && cmd.abs() > V_MAX { return Err(format!("voltage out of range [{}, {}]: {}", -1.0*V_MAX, V_MAX, cmd));}
    if mode == CmdMode::Current && cmd.abs() > I_MAX { return Err(format!("current out of range [{}, {}]: {}", -1.0*I_MAX, I_MAX, cmd));}
    gm6020_can.commands[idx] = match mode {
        CmdMode::Voltage => (V_CMD_MAX/V_MAX*cmd) as i16,
        CmdMode::Current => (I_CMD_MAX/I_MAX*cmd) as i16,
        _ => panic!(),
    };
    Ok(())
}


#[no_mangle]
pub extern "C" fn cmd_single(gm6020_can: *mut Gm6020Can, mode: CmdMode, id: u8, cmd: f64) -> i8{
    let handle: &mut Gm6020Can;
    if gm6020_can.is_null(){
        println!("Invalid handle (null pointer)");
        return -1;
    }
    else{
        handle = unsafe{&mut *gm6020_can};
    }
    _cmd_single(handle, mode, id, cmd).map_or_else(|e| {eprintln!("{}", e); -1_i8}, |_| 0_i8)
}
fn _cmd_single(gm6020_can: &mut Gm6020Can, mode: CmdMode, id: u8, cmd: f64) -> Result<(), String> {
    if id<ID_MIN || id>ID_MAX { return Err(format!("id out of range [{}, {}]: {}", ID_MIN, ID_MAX, id)); }
    set_cmd(gm6020_can, id, mode, cmd)?;
    Ok(())
}

#[no_mangle]
pub extern "C" fn cmd_multiple(gm6020_can: *mut Gm6020Can, mode: CmdMode, cmds: *const *const f64, len: u8) -> i8{
    let handle: &mut Gm6020Can;
    let cmds2: &[&[f64]]; // TODO better naming
    if gm6020_can.is_null() || cmds.is_null(){
        println!("Invalid handle or commands (null pointer)");
        return -1;
    }
    else{
        handle = unsafe{&mut *gm6020_can};
        cmds2 = unsafe {slice::from_raw_parts(cmds as *const &[f64], len as usize)} // TODO how can we assert valid data for this?
    }
    let mut cmds3: Vec<(u8, f64)> = Vec::new();
    for i in 0..(len as usize) {
        cmds3.push((cmds2[i][0 as usize] as u8, cmds2[i][1 as usize]));
    }
    _cmd_multiple(handle, mode, cmds3).map_or_else(|e| {eprintln!("{}", e); -1_i8}, |_| 0_i8)
}
fn _cmd_multiple(gm6020_can: &mut Gm6020Can, mode: CmdMode, cmds: Vec<(u8, f64)> ) -> Result<(), String> {
    for cmd in cmds.into_iter(){
        set_cmd(gm6020_can, cmd.0, mode, cmd.1)?;
    }
    Ok(())
}

fn tx_cmd(gm6020_can: &mut Gm6020Can, id_range: IdRange, mode: CmdMode) -> Result<(), String> {
    // ToDo commented for testing
    /*
    for (i, fb) in (&gm6020_can.feedbacks[((id_range as u8) * 4) as usize .. (4 + (id_range as u8)*4) as usize]).iter().enumerate() {
        if gm6020_can.commands[i] != 0 && fb.0.ok_or_else(|| Err::<(), String>(format!("Motor {} never responded. Did you enter the `run` loop?", (i as u8)+ID_MIN))).unwrap().elapsed().map_err(|err| err.to_string())?.as_millis() >= 10 {
            return Err(format!("Motor {} not responding. Did you enter the `run` loop?", (i as u8)+ID_MIN));
        }
    }*/

    let id: u16 = match (id_range, mode) {
        (IdRange::Low,  CmdMode::Voltage) => CMD_ID_V_L,
        (IdRange::High, CmdMode::Voltage) => CMD_ID_V_H,
        (IdRange::Low,  CmdMode::Current) => CMD_ID_I_L,
        (IdRange::High, CmdMode::Current) => CMD_ID_I_H,
        (_, _) => panic!(),
    };
    let cmds: &[i16] = &gm6020_can.commands[((id_range as u8) * 4) as usize .. (4 + (id_range as u8)*4) as usize];
    // TODO can we set byte alignment of commands so this can be written directly? Might need to check endian-ness
    let frame = CanFrame::new(
        StandardId::new(id).unwrap(),
        &[(cmds[0]>>8) as u8, cmds[0] as u8, (cmds[1]>>8) as u8, cmds[1] as u8, (cmds[2]>>8) as u8, cmds[2] as u8, (cmds[3]>>8) as u8, cmds[3] as u8])
        .ok_or_else(|| Err::<CanFrame, String>("Failed to open socket".to_string())).unwrap();
    gm6020_can.socket.as_ref().ok_or_else(|| Err::<CanSocket, String>("Socket not initialized".to_string())).unwrap().write_frame(&frame).map_err(|err| err.to_string())?;
    Ok(())
}

fn rx_fb(gm6020_can: &mut Gm6020Can, frame: CanDataFrame){
    println!("{:?}", frame);
    let rxid: u16 = frame.raw_id() as u16;
    let id: u8 = (rxid-FB_ID_BASE)as u8;
    if id<ID_MIN || id>ID_MAX {return;}
    let idx: usize = (id-1) as usize;
    let f: &mut (Option<SystemTime>, Feedback) = &mut gm6020_can.feedbacks[idx];
    let d: &[u8] = &frame.data()[0..8];
    f.0 = Some(SystemTime::now());// TODO waiting on socketcan library to implement hardware timestamps
    f.1.position    = (d[0] as u16) << 8 | d[1] as u16;
    f.1.speed       = (d[2] as i16) << 8 | d[3] as i16;
    f.1.current     = (d[4] as i16) << 8 | d[5] as i16;
    f.1.temperature = d[6] as u16;
    // Apparently this is frowned-upon but it looks a lot cooler
    //unsafe {
    //    f.1 = std::mem::transmute::<[u8; 8], Feedback>(frame.data()[0..8].try_into().unwrap());
    //}
    //f.1.temperature = f.1.temperature >> 8; // TODO check endianness
}
