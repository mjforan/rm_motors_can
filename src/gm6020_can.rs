use embedded_can::{Frame as EmbeddedFrame, StandardId};
use socketcan::{CanDataFrame, CanFilter, CanFrame, CanSocket, Frame, Socket, SocketOptions};
use std::ptr::null;
use std::time::SystemTime;
use std::ffi::CStr;
use std::os::raw::c_char;

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


// TODO change return types on external functions
// TODO return Option in init()
// TODO print to stderr within these functions?

/*#[no_mangle]
pub extern "C" fn init(interface: *const c_char) -> *Gm6020Can{
    if !c_string.is_null() {
        unsafe {
            // Convert the raw pointer to a CStr
            let c_str = CStr::from_ptr(c_string);

            // Convert the CStr to a &str
            if let Ok(rust_str) = c_str.to_str() {
                // Process the C-string as a Rust string
            } else {
                eprintln!("Invalid c-string received for interface name");
                return null();
            }
        }
    } else {
        println!("Invalid c-string received for interface name (null pointer)");
        return null();
    }
    /*let inter = interface.into_string();
    match _init(inter) {
        Ok(handle) => return handle;

    }*/
    return Gm6020Can::default();
}
fn _init
*/

#[no_mangle]
pub extern "C" fn init(interface: &str) -> Result<Gm6020Can, String> {
    let mut gm6020_can = Gm6020Can::default();
    gm6020_can.socket = Some(CanSocket::open(&interface).map_err(|err| err.to_string())?);

//    let frame = sock.receive().context("Receiving frame")?;

//    println!("{}  {}", iface, frame_to_string(&frame));
    // TODO set filter for feedback IDs and set async subscriber to update feedbacks array
    let filter = CanFilter::new(FB_ID_BASE as u32, 0xffffffff - 0xf);
    gm6020_can.socket.as_ref().unwrap().set_filters(&[filter]).map_err(|err| err.to_string())?;
    return Ok(gm6020_can);
}
// TODO break into separate thread for receiving
#[no_mangle]
pub extern "C" fn run(gm6020_can: &mut Gm6020Can) {
    loop {
        match gm6020_can.socket.as_ref().unwrap().read_frame() {
            Ok(CanFrame::Data(frame)) => rx_fb(gm6020_can, frame),
            Ok(CanFrame::Remote(frame)) => println!("{:?}", frame),
            Ok(CanFrame::Error(frame)) => println!("{:?}", frame),
            Err(err) => eprintln!("{}", err),
        }
    }
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
pub extern "C" fn cmd_single(gm6020_can: &mut Gm6020Can, id: u8, mode: CmdMode, cmd: f64) -> Result<(), String> {
    if id<ID_MIN || id>ID_MAX { return Err(format!("id out of range [{}, {}]: {}", ID_MIN, ID_MAX, id)); }
    set_cmd(gm6020_can, id, mode, cmd)?;
    tx_cmd(gm6020_can, match id > 4 {false=>IdRange::Low, true=>IdRange::High}, mode)?;
    Ok(())
}

#[no_mangle]
pub extern "C" fn cmd_multiple(gm6020_can: &mut Gm6020Can, mode: CmdMode, cmds: Vec<(u8, f64)> ) -> Result<(), String> {
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
}

fn tx_cmd(gm6020_can: &mut Gm6020Can, id_range: IdRange, mode: CmdMode) -> Result<(), String> {

    for (i, fb) in (&gm6020_can.feedbacks[((id_range as u8) * 4) as usize .. (4 + (id_range as u8)*4) as usize]).iter().enumerate() {
        if gm6020_can.commands[i] != 0 && fb.0.ok_or_else(|| Err::<(), String>(format!("Motor {} never responded", i))).unwrap().elapsed().map_err(|err| err.to_string())?.as_millis() >= 10 {
            return Err(format!("Motor {} not responding", i));
        }
    }

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
    f.1.position    = (d[0] as u16) << 8 | d[1] as u16; // TODO can we set byte alignment of Feedback so this can be read in directly? Might need to check endian-ness
    f.1.speed       = (d[2] as i16) << 8 | d[3] as i16;
    f.1.current     = (d[4] as i16) << 8 | d[5] as i16;
    f.1.temperature = d[6] as u16;
    //unsafe {
    //    f.1 = std::mem::transmute::<[u8; 8], Feedback>(frame.data()[0..8].try_into().unwrap());
    //}
    //f.1.temperature = f.1.temperature >> 8; // TODO check endianness
}
