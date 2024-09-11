//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

extern crate clap;
extern crate gettextrs;
extern crate plib;

use byteorder::{BigEndian, ByteOrder};
use clap::Parser;
use gettextrs::{bind_textdomain_codeset, setlocale, textdomain, LocaleCategory};
use plib::PROJECT_NAME;

use libc::{
    addrinfo, c_char, c_uchar, getaddrinfo, gethostname, getpid, getpwuid, getservbyname, getuid,
    ioctl, sa_family_t, servent, signal, sockaddr_in, winsize, AF_INET, AI_CANONNAME, SIGINT,
    SIGPIPE, SIGQUIT, SOCK_DGRAM, STDOUT_FILENO, TIOCGWINSZ,
};
use std::{
    ffi::{self, CStr, CString},
    fmt,
    fs::{self, File},
    io::{self, BufRead, BufReader, BufWriter, Cursor, Error, Write},
    mem::{size_of, zeroed},
    net::{self, Ipv4Addr, SocketAddr, TcpListener, TcpStream, UdpSocket},
    process::{self, Command},
    ptr,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

const BUFFER_SIZE: usize = 12;
const TALK_VERSION: u8 = 1;

#[derive(Debug, Copy, Clone, PartialEq)]
enum MessageType {
    LeaveInvite, // leave invitation with server
    LookUp,      // check for invitation by callee
    Delete,      // delete invitation by caller
    Announce,    // announce invitation by caller
}

#[derive(Debug, PartialEq)]
enum Answer {
    Success,          // operation completed properly
    NotHere,          // callee not logged in
    Failed,           // operation failed for unexplained reason
    MachineUnknown,   // caller's machine name unknown
    PermissionDenied, // callee's tty doesn't permit announce
    UnknownRequest,   // request has invalid type value
    BadVersion,       // request has invalid protocol version
    BadAddr,          // request has invalid addr value
    BadCtlAddr,       // request has invalid ctl_addr value
}

impl fmt::Display for Answer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match self {
            Answer::Success => "Success",
            Answer::NotHere => "Not Here",
            Answer::Failed => "Failed",
            Answer::MachineUnknown => "Machine Unknown",
            Answer::PermissionDenied => "Permission Denied",
            Answer::UnknownRequest => "Unknown Request",
            Answer::BadVersion => "Bad Version",
            Answer::BadAddr => "Bad Address",
            Answer::BadCtlAddr => "Bad Control Address",
        };
        write!(f, "{}", s)
    }
}

struct StateLogger {
    value: String,
}

impl StateLogger {
    fn new(initial_value: &str) -> Self {
        StateLogger {
            value: initial_value.to_string(),
        }
    }

    fn set_state(&mut self, new_value: &str) {
        if self.value != new_value {
            println!("{}", new_value);
            self.value = new_value.to_string();
        }
    }
}

#[repr(C, packed(1))]
pub struct Osockaddr {
    pub sa_family: sa_family_t,
    pub sa_data: [u8; 14],
}

#[repr(C, packed(1))]
struct CtlMsg {
    vers: c_uchar,
    r#type: c_uchar,
    answer: c_uchar,
    pad: c_uchar,
    id_num: u64,
    addr: Osockaddr,
    ctl_addr: Osockaddr,
    pid: i32,
    l_name: [c_char; 12],
    r_name: [c_char; 12],
    r_tty: [c_char; 12],
}

#[repr(C, packed(1))]
pub struct CtlRes {
    pub vers: c_uchar,
    r#type: MessageType,
    answer: Answer,
    pub pad: c_uchar,
    pub id_num: u64,
    pub addr: Osockaddr,
}

impl CtlMsg {
    pub fn to_bytes(&self) -> Result<Vec<u8>, io::Error> {
        let mut bytes = vec![0u8; size_of::<CtlMsg>()];
        let mut cursor = Cursor::new(&mut bytes[..]);

        cursor.write_all(&self.vers.to_be_bytes())?;
        cursor.write_all(&self.r#type.to_be_bytes())?;
        cursor.write_all(&self.answer.to_be_bytes())?;
        cursor.write_all(&self.pad.to_be_bytes())?;
        cursor.write_all(&self.id_num.to_be_bytes())?;
        cursor.write_all(&self.addr.sa_data)?;
        cursor.write_all(&self.ctl_addr.sa_data)?;
        cursor.write_all(&self.pid.to_be_bytes())?;
        cursor.write_all(&self.l_name.iter().map(|&b| b as u8).collect::<Vec<u8>>())?;
        cursor.write_all(&self.r_name.iter().map(|&b| b as u8).collect::<Vec<u8>>())?;
        cursor.write_all(&self.r_tty.iter().map(|&b| b as u8).collect::<Vec<u8>>())?;

        Ok(bytes)
    }
    pub fn create_sockaddr_data(&self, ip: &str, port: u16) -> [u8; 14] {
        let mut sa_data: [u8; 14] = [0; 14];

        let ip_bytes: Vec<u8> = ip
            .split('.')
            .map(|s| s.parse::<u8>().unwrap_or(0))
            .collect();

        if ip_bytes.len() == 4 {
            sa_data[..4].copy_from_slice(&ip_bytes);
        }

        sa_data[12..14].copy_from_slice(&port.to_be_bytes());
        sa_data
    }

    pub fn create_ctl_addr(&self, addr: SocketAddr) -> [u8; 14] {
        let mut ctl_addr: [u8; 14] = [0; 14];
        if let net::IpAddr::V4(ipv4) = addr.ip() {
            let ip_bytes = ipv4.octets();

            let port_bytes = addr.port().to_be_bytes();
            ctl_addr[0..2].copy_from_slice(&port_bytes);

            ctl_addr[2..6].copy_from_slice(&ip_bytes);
        }

        ctl_addr
    }
}

fn save_socket_addr(socket_addr: &SocketAddr, file_path: &str) -> io::Result<()> {
    let file = File::create(file_path)?;
    let mut writer = BufWriter::new(file);
    writeln!(writer, "{}", socket_addr)?;
    Ok(())
}

fn load_socket_addr(file_path: &str) -> io::Result<SocketAddr> {
    let file = File::open(file_path)?;
    let reader = io::BufReader::new(file);
    let line = reader
        .lines()
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "File is empty"))??;
    line.parse::<SocketAddr>()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// talk - talk to another user
#[derive(Parser, Debug)]
#[command(author, version, about, long_about)]
struct Args {
    /// Address to connect or listen to
    address: Option<String>,

    /// Terminal name to use (optional)
    ttyname: Option<String>,
}
fn talk(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    validate_args(&args)?;
    check_if_tty()?;

    let mut msg = initialize_ctl_msg();
    let mut res = initialize_ctl_res();

    let (width, height) = get_terminal_size();
    let mut logger = StateLogger::new("No connection yet.");

    let (my_machine_name, his_machine_name) =
        get_names(&mut msg, args.address.as_ref().unwrap(), args.ttyname)?;
    let (my_machine_addr, his_machine_addr, daemon_port) =
        get_addrs(&mut msg, &my_machine_name, &his_machine_name)?;

    let (ctl_addr, socket) = open_ctl(my_machine_addr)?;

    let ctl_addr_data = msg.create_ctl_addr(ctl_addr);
    let his_addr_data = msg.create_sockaddr_data(&his_machine_addr.to_string(), 2);
    let my_addr_data = msg.create_sockaddr_data(&my_machine_addr.to_string(), 2);

    msg.addr.sa_data = his_addr_data;
    msg.ctl_addr.sa_data = ctl_addr_data;

    look_for_invite(daemon_port, &mut msg, &socket, &mut res);

    // todo: get socket address from answer
    match load_socket_addr("socket.txt") {
        Ok(socket_addr) => {
            handle_existing_connection(
                daemon_port,
                socket_addr,
                width,
                height,
                &mut msg,
                &socket,
                &mut res,
                &mut logger,
            )?;
        }
        Err(_) => {
            handle_new_connection(
                daemon_port,
                &mut msg,
                &socket,
                &mut res,
                my_machine_addr,
                my_addr_data,
                &mut logger,
            )?;
        }
    }

    println!("[Waiting for your party to respond]");

    Ok(())
}

fn validate_args(args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    if args.address.is_none() {
        eprintln!("Usage: talk user [ttyname]");
        process::exit(-1);
    }
    Ok(())
}

fn check_if_tty() -> Result<(), Box<dyn std::error::Error>> {
    if atty::isnt(atty::Stream::Stdin) {
        println!("not a tty");
    }
    Ok(())
}

fn initialize_ctl_msg() -> CtlMsg {
    CtlMsg {
        vers: 1,
        r#type: MessageType::LookUp as u8,
        answer: Answer::Failed as u8,
        pad: 0,
        id_num: 131072,
        addr: Osockaddr {
            sa_family: 0,
            sa_data: [0; 14],
        },
        ctl_addr: Osockaddr {
            sa_family: 0,
            sa_data: [0; 14],
        },
        pid: 0,
        l_name: string_to_c_string(""),
        r_name: string_to_c_string(""),
        r_tty: [0; 12],
    }
}

fn initialize_ctl_res() -> CtlRes {
    CtlRes {
        vers: 0,
        r#type: MessageType::LookUp,
        answer: Answer::Failed,
        pad: 0,
        id_num: 0,
        addr: Osockaddr {
            sa_family: 0,
            sa_data: [0; 14],
        },
    }
}

fn handle_existing_connection(
    daemon_port: u16,
    socket_addr: SocketAddr,
    width: u16,
    height: u16,
    msg: &mut CtlMsg,
    socket: &UdpSocket,
    res: &mut CtlRes,
    logger: &mut StateLogger,
) -> Result<(), Box<dyn std::error::Error>> {
    let stream = TcpStream::connect(socket_addr)?;
    fs::remove_file("socket.txt")?;

    let mut write_stream = stream.try_clone()?;
    let read_stream = stream.try_clone()?;

    logger.set_state("[Connected to the server!]");

    let split_row = height / 2;
    send_delete(daemon_port, msg, socket, res);

    let top_line = Arc::new(Mutex::new(0 as u16));
    let bottom_line = Arc::new(Mutex::new(0 as u16));

    let top_line_clone = Arc::clone(&top_line);
    let bottom_line_clone = Arc::clone(&bottom_line);

    thread::spawn(move || {
        let mut handle = draw_terminal(split_row, width).unwrap();
        let reader = BufReader::new(read_stream);

        for line in reader.lines() {
            match line {
                Ok(message) => {
                    handle_user_input(
                        &mut handle,
                        &message,
                        split_row,
                        Arc::clone(&top_line_clone),
                        Arc::clone(&bottom_line_clone),
                    )
                    .unwrap();
                }
                Err(e) => {
                    eprintln!("Failed to receive message: {}", e);
                    break;
                }
            }
        }
    });

    let stdin = std::io::stdin();
    let handle = stdin.lock();

    for line in handle.lines() {
        let message = line.unwrap();
        let mut top_line = top_line.lock().unwrap();
        *top_line += 1;
        write_stream.write_all(message.as_bytes())?;
        write_stream.write_all(b"\n")?;
        if *top_line >= split_row - 1 {
            eprint!("\x1B[{};H", 0);
            *top_line = 0;
        }
    }

    Ok(())
}

fn handle_new_connection(
    daemon_port: u16,
    msg: &mut CtlMsg,
    socket: &UdpSocket,
    res: &mut CtlRes,
    my_machine_addr: Ipv4Addr,
    my_addr_data: [u8; 14],
    logger: &mut StateLogger,
) -> Result<(), Box<dyn std::error::Error>> {
    let (socket_addr, listener) = open_sockt(my_machine_addr)?;
    save_socket_addr(&socket_addr, "socket.txt")?;

    logger.set_state("[Service connection established.]");

    msg.addr.sa_data = my_addr_data;
    leave_invite(daemon_port, msg, socket, res);
    announce(daemon_port, msg, socket, res);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                handle_client(stream).unwrap();
            }
            Err(e) => {
                eprintln!("Failed to accept connection: {}", e);
            }
        }
    }

    Ok(())
}

// Determine the local and remote user, tty, and machines
fn get_names(
    msg: &mut CtlMsg,
    address: &str,
    ttyname: Option<String>,
) -> Result<(String, String), io::Error> {
    // Get the current user's name
    let my_name = unsafe {
        let login_name = libc::getlogin();
        if !login_name.is_null() {
            CStr::from_ptr(login_name).to_string_lossy().into_owned()
        } else {
            let pw = getpwuid(getuid());
            if pw.is_null() {
                return Err(Error::new(
                    io::ErrorKind::NotFound,
                    "You don't exist. Go away.",
                ));
            } else {
                CStr::from_ptr((*pw).pw_name).to_string_lossy().into_owned()
            }
        }
    };

    // Get the local machine name
    // todo: allocate enought sized buffer - safety
    let my_machine_name = {
        let mut buffer = vec![0 as c_char; 256];
        let result = unsafe { gethostname(buffer.as_mut_ptr(), buffer.len()) };

        if result == 0 {
            let c_str = unsafe { CStr::from_ptr(buffer.as_ptr()) };
            c_str.to_string_lossy().into_owned()
        } else {
            return Err(Error::new(
                io::ErrorKind::Other,
                "Cannot get local hostname",
            ));
        }
    };

    let have_at_symbol = address.find(|c| "@:!.".contains(c));

    let (his_name, his_machine_name) = if let Some(index) = have_at_symbol {
        let delimiter = address.chars().nth(index).unwrap();
        if delimiter == '@' {
            /* user@host */
            let (user, host) = address.split_at(index);
            (user.to_string(), host[1..].to_string())
        } else {
            /* host.user or host!user or host:user */
            let (host, user) = address.split_at(index);
            (user[1..].to_string(), host.to_string())
        }
    } else {
        // local for local talk
        (address.to_string(), my_machine_name.clone())
    };

    msg.vers = TALK_VERSION;
    msg.addr.sa_family = AF_INET as sa_family_t;
    msg.ctl_addr.sa_family = AF_INET as sa_family_t;
    //todo: use id_num properly
    // msg.id_num = 0u32.to_be() as u64;
    msg.l_name = string_to_c_string(&my_name);
    msg.r_name = string_to_c_string(&his_name);
    msg.r_tty = string_to_c_string(&ttyname.unwrap_or_default());

    Ok((my_machine_name, his_machine_name))
}

fn get_addrs(
    msg: &mut CtlMsg,
    my_machine_name: &str,
    his_machine_name: &str,
) -> Result<(Ipv4Addr, Ipv4Addr, u16), std::io::Error> {
    let service = CString::new("ntalk")?;
    let protocol = CString::new("udp")?;
    //todo: add IDN
    let lhost = CString::new(my_machine_name)?;
    let rhost = CString::new(his_machine_name)?;

    let mut my_machine_addr: Ipv4Addr = Ipv4Addr::UNSPECIFIED;
    let mut his_machine_addr: Ipv4Addr = Ipv4Addr::UNSPECIFIED;

    msg.pid = unsafe { getpid() };

    let hints = addrinfo {
        ai_family: AF_INET, // IPv4 only
        ai_socktype: SOCK_DGRAM,
        ai_flags: AI_CANONNAME,
        ai_protocol: 0,
        ai_addrlen: 0,
        ai_canonname: ptr::null_mut(),
        ai_addr: ptr::null_mut(),
        ai_next: ptr::null_mut(),
    };

    unsafe {
        // Get local host address
        let mut res: *mut addrinfo = ptr::null_mut();
        let err = getaddrinfo(lhost.as_ptr(), ptr::null(), &hints, &mut res);
        if err != 0 {
            eprintln!(
                "talk: {}: {}",
                my_machine_name,
                std::ffi::CStr::from_ptr(libc::gai_strerror(err))
                    .to_str()
                    .unwrap()
            );
            process::exit(-1);
        }

        let mut ai = res;
        while !ai.is_null() {
            let ai_ref = &*ai;
            if ai_ref.ai_family == AF_INET {
                let sockaddr: &sockaddr_in = &*(ai_ref.ai_addr as *const sockaddr_in);
                my_machine_addr = Ipv4Addr::from(u32::from_be(sockaddr.sin_addr.s_addr));
                break;
            }
            ai = ai_ref.ai_next;
        }

        if my_machine_addr == Ipv4Addr::UNSPECIFIED {
            eprintln!("talk: {}: address not found", my_machine_name);
            process::exit(-1);
        }

        // Get remote host address
        if rhost != lhost {
            let mut res_remote: *mut addrinfo = ptr::null_mut();
            let err_remote = getaddrinfo(rhost.as_ptr(), ptr::null(), &hints, &mut res_remote);
            if err_remote != 0 {
                eprintln!(
                    "talk: {}: {}",
                    his_machine_name,
                    ffi::CStr::from_ptr(libc::gai_strerror(err_remote))
                        .to_str()
                        .unwrap()
                );
                process::exit(-1);
            }

            let mut ai_remote = res_remote;
            while !ai_remote.is_null() {
                let ai_ref = &*ai_remote;
                if ai_ref.ai_family == AF_INET {
                    let sockaddr: &sockaddr_in = &*(ai_ref.ai_addr as *const sockaddr_in);
                    his_machine_addr = Ipv4Addr::from(u32::from_be(sockaddr.sin_addr.s_addr));
                    break;
                }
                ai_remote = ai_ref.ai_next;
            }

            if his_machine_addr == Ipv4Addr::UNSPECIFIED {
                eprintln!("talk: {}: address not found", his_machine_name);
                process::exit(-1);
            }
        } else {
            his_machine_addr = my_machine_addr;
        }
    }

    let talkd_service: *mut servent = unsafe { getservbyname(service.as_ptr(), protocol.as_ptr()) };

    if talkd_service.is_null() {
        eprintln!("talk: {}/{}: service is not registered.", "ntalk", "udp");
        std::process::exit(1);
    }

    let daemon_port = unsafe {
        let servent = *talkd_service;
        let port = servent.s_port;
        u16::from_be(port as u16)
    };

    Ok((my_machine_addr, his_machine_addr, daemon_port))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // parse command line arguments
    let args = Args::parse();

    setlocale(LocaleCategory::LcAll, "");
    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;

    let mut exit_code = 0;

    register_signals();

    if let Err(err) = talk(args) {
        exit_code = 1;
        eprint!("{}", err);
    }

    process::exit(exit_code)
}

fn string_to_c_string(s: &str) -> [c_char; BUFFER_SIZE] {
    let mut buffer: [c_char; BUFFER_SIZE] = [0; BUFFER_SIZE];
    let c_string = CString::new(s).expect("CString::new failed");
    let bytes = c_string.to_bytes();

    for (i, &byte) in bytes.iter().take(BUFFER_SIZE - 1).enumerate() {
        buffer[i] = byte as c_char;
    }
    buffer
}

fn bytes_to_ctl_res(bytes: &[u8]) -> CtlRes {
    let id_num = BigEndian::read_u64(&bytes[4..12]);

    let sa_family = bytes[12] as sa_family_t;
    let addr = Osockaddr {
        sa_family,
        sa_data: [0; 14],
    };

    let r#type = match bytes[1] {
        0 => MessageType::LeaveInvite,
        1 => MessageType::LookUp,
        2 => MessageType::Delete,
        3 => MessageType::Announce,
        _ => MessageType::LookUp, // Default or error handling
    };

    let answer = match bytes[2] {
        0 => Answer::Success,
        1 => Answer::NotHere,
        2 => Answer::Failed,
        3 => Answer::MachineUnknown,
        4 => Answer::PermissionDenied,
        5 => Answer::UnknownRequest,
        6 => Answer::BadVersion,
        7 => Answer::BadAddr,
        8 => Answer::BadCtlAddr,
        _ => Answer::Failed, // Default or error handling
    };

    CtlRes {
        vers: bytes[0],
        r#type,
        answer,
        pad: bytes[3],
        id_num,
        addr,
    }
}

fn handle_client(stream: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    let read_stream = stream.try_clone()?;
    let mut write_stream = stream.try_clone()?;

    let (width, height) = get_terminal_size();
    let split_row = height / 2;
    // Wrap top_line and bottom_line in Arc<Mutex<_>> for shared mutable access
    let top_line = Arc::new(Mutex::new(0 as u16));
    let bottom_line = Arc::new(Mutex::new(0 as u16));

    // Clone Arcs for the thread
    let top_line_clone = Arc::clone(&top_line);
    let bottom_line_clone = Arc::clone(&bottom_line);

    // Spawn a thread to handle incoming messages from the server
    thread::spawn(move || {
        let mut handle = draw_terminal(split_row, width).unwrap();
        let reader = BufReader::new(read_stream);

        for line in reader.lines() {
            match line {
                Ok(message) => {
                    handle_user_input(
                        &mut handle,
                        &message,
                        split_row,
                        Arc::clone(&top_line_clone),
                        Arc::clone(&bottom_line_clone),
                    )
                    .unwrap();
                }
                Err(e) => {
                    eprintln!("Failed to receive message: {}", e);
                    break;
                }
            }
        }
    });

    // Main thread handles stdin input
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let message = line.unwrap();
        let mut top_line = top_line.lock().unwrap();
        *top_line += 1;
        write_stream.write_all(message.as_bytes())?;
        write_stream.write_all(b"\n")?;
        if *top_line >= split_row - 1 {
            eprint!("\x1B[{};H", 0);
            *top_line = 0;
        }
    }
    Ok(())
}

fn handle_user_input(
    handle: &mut io::StdoutLock,
    input: &str,
    split_row: u16,
    top_line: Arc<Mutex<u16>>,
    bottom_line: Arc<Mutex<u16>>,
) -> io::Result<()> {
    let top_line = top_line.lock().unwrap();
    let mut bottom_line = bottom_line.lock().unwrap();

    if *bottom_line >= split_row - 2 {
        *bottom_line = 0; // Reset bottom_line if it exceeds split_row
    }

    write!(handle, "\x1b[{};0H", split_row + *bottom_line + 1)?; // Move cursor to bottom window
    writeln!(handle, "{}", input)?;
    write!(handle, "\n")?;

    write!(handle, "\x1b[{};0H", *top_line)?; // Move cursor to top window
    handle.flush()?;
    *bottom_line += 1;
    Ok(())
}
fn draw_terminal(split_row: u16, width: u16) -> io::Result<io::StdoutLock<'static>> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();

    // Clear the terminal
    Command::new("clear").status()?;

    // Draw the split line
    write!(handle, "\x1b[{};0H", split_row)?;
    writeln!(handle, "└{:─<width$}┘", "", width = (width as usize) - 2)?;

    write!(handle, "\x1b[0;0H")?;
    handle.flush()?;

    Ok(handle)
}
fn open_sockt(my_machine_addr: Ipv4Addr) -> Result<(SocketAddr, TcpListener), io::Error> {
    let listener = TcpListener::bind((my_machine_addr, 0))?;
    let addr = listener.local_addr()?;

    Ok((addr, listener))
}

fn open_ctl(my_machine_addr: Ipv4Addr) -> Result<(SocketAddr, UdpSocket), io::Error> {
    let socket = UdpSocket::bind((my_machine_addr, 0))?;

    let addr = socket.local_addr()?;

    Ok((addr, socket))
}

// Unified invite handler with reduced arguments
fn handle_invite(
    daemon_port: u16,
    msg: &mut CtlMsg,
    socket: &UdpSocket,
    res: &mut CtlRes,
    msg_type: MessageType,
) {
    if let Err(e) = reqwest(daemon_port, msg, msg_type, socket, res) {
        eprint!("Error handling {:?} message: {}\n", msg_type, e);
    }
}

// Simplified function calls
fn look_for_invite(daemon_port: u16, msg: &mut CtlMsg, socket: &UdpSocket, res: &mut CtlRes) {
    handle_invite(daemon_port, msg, socket, res, MessageType::LookUp);
}

fn leave_invite(daemon_port: u16, msg: &mut CtlMsg, socket: &UdpSocket, res: &mut CtlRes) {
    handle_invite(daemon_port, msg, socket, res, MessageType::LeaveInvite);
}

fn announce(daemon_port: u16, msg: &mut CtlMsg, socket: &UdpSocket, res: &mut CtlRes) {
    handle_invite(daemon_port, msg, socket, res, MessageType::Announce);
}

fn send_delete(daemon_port: u16, msg: &mut CtlMsg, socket: &UdpSocket, res: &mut CtlRes) {
    handle_invite(daemon_port, msg, socket, res, MessageType::Delete);
}

// Improved reqwest function with eprint for error reporting
fn reqwest(
    daemon_port: u16,
    msg: &mut CtlMsg,
    msg_type: MessageType,
    socket: &UdpSocket,
    res: &mut CtlRes,
) -> std::io::Result<()> {
    let talkd_addr: SocketAddr = format!("0.0.0.0:{}", daemon_port).parse().unwrap();

    msg.r#type = msg_type as u8;
    let msg_bytes = msg.to_bytes()?;

    loop {
        match socket.send_to(&msg_bytes, talkd_addr) {
            Ok(_) => {
                let mut buf = [0; 1024];
                match socket.recv_from(&mut buf) {
                    Ok((amt, _)) => {
                        let ctl_res = bytes_to_ctl_res(&buf[..amt]);
                        res.answer = ctl_res.answer;
                        res.r#type = ctl_res.r#type;
                        break;
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                    Err(e) => {
                        eprint!("Error receiving message: {}\n", e);
                        return Err(e);
                    }
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_secs(1));
            }
            Err(e) => {
                eprint!("Error sending message: {}\n", e);
                return Err(e);
            }
        }
    }
    Ok(())
}

/// Handles incoming signals by setting the interrupt flag and exiting the process.
pub fn handle_signals(signal_code: libc::c_int) {
    std::process::exit(128 + signal_code);
}

pub fn register_signals() {
    unsafe {
        signal(SIGINT, handle_signals as usize);
        signal(SIGQUIT, handle_signals as usize);
        signal(SIGPIPE, handle_signals as usize);
    }
}

fn get_terminal_size() -> (u16, u16) {
    let mut size: winsize = unsafe { zeroed() };

    // Get the terminal size using ioctl
    unsafe {
        ioctl(STDOUT_FILENO, TIOCGWINSZ, &mut size);
    }

    (size.ws_col, size.ws_row)
}
