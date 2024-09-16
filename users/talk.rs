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

use clap::Parser;
use gettextrs::{bind_textdomain_codeset, setlocale, textdomain, LocaleCategory};
use plib::PROJECT_NAME;

#[cfg(target_os = "linux")]
use libc::sa_family_t;
use libc::{
    addrinfo, c_char, c_uchar, getaddrinfo, gethostname, getpid, getpwuid, getservbyname, getuid,
    ioctl, servent, signal, sockaddr_in, winsize, AF_INET, AI_CANONNAME, SIGINT, SIGPIPE, SIGQUIT,
    SOCK_DGRAM, STDOUT_FILENO, TIOCGWINSZ,
};
use std::{
    ffi::{self, CStr, CString},
    fs::{remove_file, File},
    io::{self, BufRead, Cursor, Error, ErrorKind, Read, Write},
    mem::{size_of, zeroed},
    net::{self, Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream, UdpSocket},
    process::{self, Command},
    ptr,
    str::from_utf8,
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

impl TryFrom<u8> for MessageType {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(MessageType::LeaveInvite),
            1 => Ok(MessageType::LookUp),
            2 => Ok(MessageType::Delete),
            3 => Ok(MessageType::Announce),
            _ => Err(()),
        }
    }
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

impl TryFrom<u8> for Answer {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Answer::Success),
            1 => Ok(Answer::NotHere),
            2 => Ok(Answer::Failed),
            3 => Ok(Answer::MachineUnknown),
            4 => Ok(Answer::PermissionDenied),
            5 => Ok(Answer::UnknownRequest),
            6 => Ok(Answer::BadVersion),
            7 => Ok(Answer::BadAddr),
            8 => Ok(Answer::BadCtlAddr),
            _ => Err(()),
        }
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

#[cfg(target_os = "macos")]
type SaFamily = u16;

#[cfg(target_os = "linux")]
type SaFaily = sa_family_t;

#[repr(C, packed)]
pub struct Osockaddr {
    pub sa_family: SaFamily,
    pub sa_data: [u8; 14],
}

impl Osockaddr {
    pub fn to_socketaddr(&self) -> Option<SocketAddrV4> {
        // Extract the port (first 2 bytes, big-endian)
        let port = u16::from_be_bytes([self.sa_data[0], self.sa_data[1]]);

        // Extract the IP address (next 4 bytes)
        let ip = Ipv4Addr::new(
            self.sa_data[2],
            self.sa_data[3],
            self.sa_data[4],
            self.sa_data[5],
        );

        Some(SocketAddrV4::new(ip, port))
    }
}

#[repr(C, packed)]
struct CtlMsg {
    vers: c_uchar,
    r#type: c_uchar,
    answer: c_uchar,
    pad: c_uchar,
    id_num: u32,
    addr: Osockaddr,
    ctl_addr: Osockaddr,
    pid: i32,
    l_name: [c_char; 12],
    r_name: [c_char; 12],
    r_tty: [c_char; 16],
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
        cursor.write_all(&self.addr.sa_family.to_be_bytes())?;
        cursor.write_all(&self.addr.sa_data)?;
        cursor.write_all(&self.ctl_addr.sa_family.to_be_bytes())?;
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

        sa_data[0..2].copy_from_slice(&port.to_be_bytes());
        sa_data[2..6].copy_from_slice(&ip_bytes);
        sa_data[12..14].copy_from_slice(&[0, 2]);

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

#[repr(C, packed)]
pub struct CtlRes {
    pub vers: c_uchar,
    r#type: MessageType,
    answer: Answer,
    pub pad: c_uchar,
    pub id_num: u32,
    pub addr: Osockaddr,
}

impl CtlRes {
    fn from_bytes(&self, bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() < size_of::<CtlRes>() {
            return Err("Not enough data to form CtlRes");
        }

        let vers = bytes[0];
        let r#type = MessageType::try_from(bytes[1]).map_err(|_| "Invalid MessageType")?;
        let answer = Answer::try_from(bytes[2]).map_err(|_| "Invalid Answer")?;
        let pad = bytes[3];
        let id_num = u32::from_le_bytes(bytes[4..8].try_into().unwrap());

        let sa_family = u16::from_le_bytes(bytes[8..10].try_into().unwrap());
        let mut sa_data = [0u8; 14];
        sa_data.copy_from_slice(&bytes[10..24]);

        let addr = Osockaddr { sa_family, sa_data };

        Ok(CtlRes {
            vers,
            r#type,
            answer,
            pad,
            id_num,
            addr,
        })
    }
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
    let (my_machine_addr, _his_machine_addr, daemon_port) =
        get_addrs(&mut msg, &my_machine_name, &his_machine_name)?;

    let (ctl_addr, socket) = open_ctl(my_machine_addr)?;

    let ctl_addr_data = msg.create_ctl_addr(ctl_addr);

    msg.ctl_addr.sa_data = ctl_addr_data;

    logger.set_state("[Checking for invitation on caller's machine]");
    look_for_invite(daemon_port, &mut msg, &socket, &mut res);
    msg.id_num = res.id_num.to_be();
    send_delete(daemon_port, &mut msg, &socket, &mut res);

    if res.answer == Answer::Success {
        handle_existing_connection(width, height, &mut res, daemon_port, &mut msg, &socket)?;
    } else {
        logger.set_state("[Waiting to connect with caller]");
        handle_new_connection(
            daemon_port,
            &mut msg,
            &socket,
            &mut res,
            my_machine_addr,
            &mut logger,
        )?;
    }

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
        // println!("not a tty");
    }
    Ok(())
}

fn initialize_ctl_msg() -> CtlMsg {
    CtlMsg {
        vers: 1,
        r#type: MessageType::LookUp as u8,
        answer: Answer::Success as u8,
        pad: 0,
        id_num: 0,
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
        r_tty: [0; 16],
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
    width: u16,
    height: u16,
    res: &mut CtlRes,
    daemon_port: u16,
    msg: &mut CtlMsg,
    socket: &UdpSocket,
) -> Result<(), io::Error> {
    let tcp_addr = res.addr.to_socketaddr().unwrap();

    let stream = TcpStream::connect(tcp_addr)?;
    let (local_id, remote_id) = read_invite_ids_from_file()?;
    msg.id_num = local_id;
    send_delete(daemon_port, msg, socket, res);
    msg.id_num = remote_id;
    send_delete(daemon_port, msg, socket, res);

    remove_file("invite_ids.txt")?;

    let mut write_stream = stream.try_clone()?;
    let read_stream = stream.try_clone()?;

    let split_row = height / 2;

    let top_line = Arc::new(Mutex::new(2 as u16));
    let bottom_line = Arc::new(Mutex::new(0 as u16));

    let top_line_clone = Arc::clone(&top_line);
    let bottom_line_clone = Arc::clone(&bottom_line);

    thread::spawn(move || {
        let mut handle = draw_terminal(split_row, width).unwrap();
        let mut buffer = [0; 128];
        let mut stream = read_stream;
        loop {
            match stream.read(&mut buffer) {
                Ok(nbytes) => {
                    if nbytes > 0 {
                        handle_user_input(
                            &mut handle,
                            std::str::from_utf8(&buffer[..nbytes]).unwrap(),
                            split_row,
                            Arc::clone(&top_line_clone),
                            Arc::clone(&bottom_line_clone),
                        )
                        .unwrap();
                    } else {
                        Command::new("clear").status().unwrap();

                        eprintln!("Connection closed, exiting...");
                        process::exit(128);
                    }
                }
                Err(e) => {
                    eprintln!("Error reading from stream: {}", e);
                    break;
                }
            }
        }
    });

    let stdin = std::io::stdin();
    let handle = stdin.lock();

    for line in handle.lines() {
        match line {
            Ok(message) => {
                let mut top_line = top_line.lock().unwrap();
                *top_line += 1;
                if let Err(e) = write_stream.write_all(message.as_bytes()) {
                    eprintln!("Failed to send message: {}", e);
                    break;
                }
                if let Err(e) = write_stream.write_all(b"\n") {
                    eprintln!("Failed to send newline: {}", e);
                    break;
                }
                if *top_line >= split_row.checked_sub(1).unwrap_or(0) {
                    eprint!("\x1B[{};H", 1);
                    *top_line = 1;
                }
            }
            Err(e) => {
                eprintln!("Failed to read from stdin: {}", e);
                break;
            }
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
    logger: &mut StateLogger,
) -> Result<(), Box<dyn std::error::Error>> {
    let (socket_addr, listener) = open_sockt(my_machine_addr)?;

    logger.set_state("[Service connection established.]");

    let tcp_data = msg.create_sockaddr_data(&socket_addr.ip().to_string(), socket_addr.port());

    msg.addr.sa_data = tcp_data;
    logger.set_state("[Waiting for your party to respond]");
    announce(daemon_port, msg, socket, res);
    let remote_id: u32 = res.id_num;
    leave_invite(daemon_port, msg, socket, res);
    let local_id: u32 = res.id_num;

    save_invite_ids_to_file(local_id, remote_id)?;

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                handle_client(stream).unwrap();
            }
            Err(e) if e.kind() == ErrorKind::UnexpectedEof => {
                println!(
                    ")Connection closed gracefully by peer (reported as ErrorKind::UnexpectedEof)"
                );
                break;
            }
            Err(e) if e.kind() == ErrorKind::ConnectionAborted => {
                println!("ErrorKind::ConnectionAborted");
                break;
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

    msg.addr.sa_family = AF_INET as SaFamily;
    msg.ctl_addr.sa_family = AF_INET as SaFamily;
    msg.l_name = string_to_c_string(&my_name);
    msg.r_name = string_to_c_string(&his_name);
    // msg.r_tty = string_to_c_string(&ttyname.unwrap_or_default());
    msg.r_tty = [0; 16];

    Ok((my_machine_name, his_machine_name))
}

fn get_addrs(
    msg: &mut CtlMsg,
    my_machine_name: &str,
    his_machine_name: &str,
) -> Result<(Ipv4Addr, Ipv4Addr, u16), std::io::Error> {
    let service = CString::new("ntalk")?;
    let protocol = CString::new("udp")?;
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
fn handle_client(stream: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    let read_stream = stream.try_clone()?;
    let mut write_stream = stream.try_clone()?;

    let (width, height) = get_terminal_size();
    let split_row = height / 2;
    let top_line = Arc::new(Mutex::new(2 as u16));
    let bottom_line = Arc::new(Mutex::new(0 as u16));

    let top_line_clone = Arc::clone(&top_line);
    let bottom_line_clone = Arc::clone(&bottom_line);

    thread::spawn(move || {
        let mut handle = draw_terminal(split_row, width).unwrap();
        let mut buffer = [0; 128];
        let mut stream = stream;
        loop {
            match stream.read(&mut buffer) {
                Ok(nbytes) => {
                    if nbytes > 0 {
                        handle_user_input(
                            &mut handle,
                            from_utf8(&buffer[..nbytes]).unwrap(),
                            split_row,
                            Arc::clone(&top_line_clone),
                            Arc::clone(&bottom_line_clone),
                        )
                        .unwrap();
                    } else {
                        Command::new("clear").status().unwrap();

                        eprintln!("Connection closed, exiting...");
                        std::process::exit(128);
                    }
                }
                Err(e) => {
                    println!("ErrorKind: {e}");
                    break;
                }
            }
        }
    });

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let message = line.unwrap();
        let mut top_line = top_line.lock().unwrap();
        *top_line += 1;
        write_stream.write_all(message.as_bytes())?;
        write_stream.write_all(b"\n")?;
        if *top_line >= split_row.checked_sub(1).unwrap_or(0) {
            eprint!("\x1B[{};H", 1);
            *top_line = 1;
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
        *bottom_line = 0;
    }

    // Move cursor to bottom window
    write!(handle, "\x1b[{};0H", split_row + *bottom_line + 1)?;
    writeln!(handle, "{}", input)?;

    // Move cursor to top window
    write!(handle, "\x1b[{};H", *top_line)?;
    handle.flush()?;
    *bottom_line += 1;
    Ok(())
}
fn draw_terminal(split_row: u16, width: u16) -> io::Result<io::StdoutLock<'static>> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();

    Command::new("clear").status()?;
    write!(handle, "[Connection established]")?;

    // Draw the split line
    write!(handle, "\x1b[{};0H", split_row)?;

    writeln!(
        handle,
        "└{:─<width$}┘",
        "",
        width = (width as usize).checked_sub(2).unwrap_or(0)
    )?;

    write!(handle, "\x1b[1;H")?;
    write!(handle, "\x1B[1B")?;
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
                        let ctl_res = res.from_bytes(&buf[..amt]).unwrap();
                        *res = ctl_res;
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
fn save_invite_ids_to_file(local_id: u32, remote_id: u32) -> io::Result<()> {
    let mut file = File::create("invite_ids.txt")?;
    writeln!(file, "local_id={}", local_id)?;
    writeln!(file, "remote_id={}", remote_id)?;
    Ok(())
}
/// Handles incoming signals by setting the interrupt flag and exiting the process.
pub fn handle_signals(signal_code: libc::c_int) {
    if let Err(e) = Command::new("clear").status() {
        eprintln!("Failed to clear the terminal: {}", e);
    }
    eprintln!("Connection closed, exiting...");

    std::process::exit(128 + signal_code);
}

fn read_invite_ids_from_file() -> io::Result<(u32, u32)> {
    let file = File::open("invite_ids.txt")?;
    let reader = io::BufReader::new(file);

    let mut local_id = None;
    let mut remote_id = None;

    for line in reader.lines() {
        let line = line?;
        if let Some((key, value)) = line.split_once('=') {
            match key {
                "local_id" => local_id = value.parse().ok(),
                "remote_id" => remote_id = value.parse().ok(),
                _ => {}
            }
        }
    }

    Ok((
        local_id.ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "local_id not found"))?,
        remote_id.ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "remote_id not found"))?,
    ))
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

    unsafe {
        ioctl(STDOUT_FILENO, TIOCGWINSZ, &mut size);
    }

    (size.ws_col, size.ws_row)
}
