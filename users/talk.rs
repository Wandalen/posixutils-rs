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
    ioctl, signal, sockaddr_in, winsize, AF_INET, AI_CANONNAME, SIGINT, SIGPIPE, SIGQUIT,
    SOCK_DGRAM, STDOUT_FILENO, TIOCGWINSZ,
};
use std::{
    ffi::{CStr, CString},
    fs::{remove_file, File},
    io::{self, BufRead, Cursor, Error, Read, Write},
    mem::{size_of, zeroed},
    net::{
        self, AddrParseError, Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream, UdpSocket,
    },
    process, ptr,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

// The size of the buffer for control message fields like l_name, r_name, and r_tty in CtlMsg.
const BUFFER_SIZE: usize = 12;

// The maximum size for the buffer to store the hostname, based on typical hostname lengths.
const HOSTNAME_BUFFER_SIZE: usize = 256;

// The maximum number of characters allowed for user input in a single operation.
const MAX_USER_INPUT_LENGTH: usize = 128;
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

#[derive(Debug)]
pub enum TalkError {
    InvalidArguments,
    NotTty,
    AddressResolutionFailed(String),
    IoError(io::Error),
    Other(String),
}

impl std::error::Error for TalkError {}

impl std::fmt::Display for TalkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TalkError::InvalidArguments => {
                write!(f, "Usage: talk user [ttyname]")?;
                process::exit(-1)
            }
            TalkError::NotTty => write!(f, "Not a TTY"),
            TalkError::AddressResolutionFailed(msg) => {
                write!(f, "Failed to resolve addresses: {}", msg)
            }
            TalkError::IoError(e) => write!(f, "I/O error: {}", e),
            TalkError::Other(msg) => write!(f, "An error occurred: {}", msg),
        }
    }
}

#[cfg(target_os = "macos")]
type SaFamily = u16;

#[cfg(target_os = "linux")]
type SaFamily = sa_family_t;

#[repr(C, packed)]
pub struct Osockaddr {
    pub sa_family: SaFamily,
    pub sa_data: [u8; 14],
}

impl Osockaddr {
    // Converts the packed address structure into a SocketAddrV4
    pub fn to_socketaddr(&self) -> Option<SocketAddrV4> {
        // Extract the port
        let port = u16::from_be_bytes([self.sa_data[0], self.sa_data[1]]);

        // Extract the IP address
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
    pub fn initialize() -> Self {
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

    // Converts the CtlMsg structure into a vector of bytes for network transmission
    fn to_bytes(&self) -> Result<Vec<u8>, io::Error> {
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
    // create sockaddr data from IP and port
    fn create_sockaddr_data(&self, ip: &str, port: u16) -> [u8; 14] {
        let mut sa_data: [u8; 14] = [0; 14];

        let ip_segments: Result<Vec<u8>, _> = ip.split('.').map(|s| s.parse::<u8>()).collect();

        match ip_segments {
            Ok(ip_bytes) if ip_bytes.len() == 4 => {
                sa_data[0..2].copy_from_slice(&port.to_be_bytes());
                sa_data[2..6].copy_from_slice(&ip_bytes);
                sa_data[12..14].copy_from_slice(&[0, 2]);
            }
            _ => {
                eprint!("Invalid IP address format: {}", ip);
            }
        }

        sa_data
    }
    // create control sockaddr data from a SocketAddr
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
    pub fn initialize() -> Self {
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

    // Converts a byte slice into a CtlRes struct, ensuring correct parsing of each field.
    fn from_bytes(bytes: &[u8]) -> Result<Self, &'static str> {
        use std::convert::TryInto;
        use std::mem::size_of;

        // Check if the input data has enough bytes to form a valid CtlRes
        if bytes.len() < size_of::<CtlRes>() {
            return Err("Not enough data to form CtlRes");
        }

        // Extract version byte
        let vers = *bytes.get(0).ok_or("Missing version byte")?;

        // Extract and validate MessageType
        let r#type = MessageType::try_from(*bytes.get(1).ok_or("Missing MessageType byte")?)
            .map_err(|_| "Invalid MessageType")?;

        // Extract and validate Answer
        let answer = Answer::try_from(*bytes.get(2).ok_or("Missing Answer byte")?)
            .map_err(|_| "Invalid Answer")?;

        // Extract padding byte
        let pad = *bytes.get(3).ok_or("Missing padding byte")?;

        // Extract id_num (4 bytes)
        let id_num = bytes
            .get(4..8)
            .ok_or("Missing id_num bytes")?
            .try_into()
            .map(u32::from_le_bytes)
            .map_err(|_| "Failed to parse id_num")?;

        // Extract sa_family (2 bytes)
        let sa_family = bytes
            .get(8..10)
            .ok_or("Missing sa_family bytes")?
            .try_into()
            .map(u16::from_le_bytes)
            .map_err(|_| "Failed to parse sa_family")?;

        // Extract and copy sa_data (14 bytes)
        let mut sa_data = [0u8; 14];
        bytes
            .get(10..24)
            .ok_or("Missing sa_data bytes")?
            .try_into()
            .map(|slice: &[u8; 14]| sa_data.copy_from_slice(slice))
            .map_err(|_| "Failed to copy sa_data")?;

        // Create Osockaddr with extracted sa_family and sa_data
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
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

fn talk(args: Args) -> Result<(), TalkError> {
    validate_args(&args)?;
    check_if_tty()?;

    let mut msg = CtlMsg::initialize();
    let mut res = CtlRes::initialize();
    let (width, height) = get_terminal_size();

    let mut logger = StateLogger::new("No connection yet.");

    // Retrieve the local and remote machine names
    let (my_machine_name, his_machine_name) = get_names(
        &mut msg,
        args.address
            .as_ref()
            .ok_or_else(|| TalkError::InvalidArguments)?,
        args.ttyname,
    )
    .map_err(|e| TalkError::IoError(e))?;

    // Get the local and remote addresses, and the daemon port number
    let (my_machine_addr, _his_machine_addr, daemon_port) =
        get_addrs(&mut msg, &my_machine_name, &his_machine_name)
            .map_err(|e| TalkError::IoError(e))?;

    // Open control socket
    let (ctl_addr, socket) = open_ctl(my_machine_addr).map_err(|e| TalkError::IoError(e))?;

    let ctl_addr_data = msg.create_ctl_addr(ctl_addr);
    msg.ctl_addr.sa_data = ctl_addr_data;

    logger.set_state("[Checking for invitation on caller's machine]");

    // Look for an invitation from the daemon
    look_for_invite(daemon_port, &mut msg, &socket, &mut res)?;

    // Set the invitation ID number and send a delete request for the old invitation
    msg.id_num = res.id_num.to_be();
    send_delete(daemon_port, &mut msg, &socket, &mut res)?;

    if res.answer == Answer::Success {
        handle_existing_invitation(width, height, &mut res, daemon_port, &mut msg, &socket)?;
    } else {
        logger.set_state("[Waiting to connect with caller]");
        handle_new_invitation(
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

fn validate_args(args: &Args) -> Result<(), TalkError> {
    if args.address.is_none() {
        return Err(TalkError::InvalidArguments);
    }
    Ok(())
}

fn check_if_tty() -> Result<(), TalkError> {
    if atty::isnt(atty::Stream::Stdin) {
        eprintln!("Not a TTY");
        return Err(TalkError::NotTty);
    }
    Ok(())
}

fn handle_existing_invitation(
    width: u16,
    height: u16,
    res: &mut CtlRes,
    daemon_port: u16,
    msg: &mut CtlMsg,
    socket: &UdpSocket,
) -> Result<(), TalkError> {
    let tcp_addr = res.addr.to_socketaddr().ok_or_else(|| {
        TalkError::AddressResolutionFailed("Failed to convert address to socket address.".into())
    })?;

    // Establish a TCP connection to the `tcp_addr`. Map any IO errors to `TalkError::IoError`.
    let stream = TcpStream::connect(tcp_addr).map_err(TalkError::IoError)?;
    let (local_id, remote_id) = read_invite_ids_from_file().map_err(TalkError::IoError)?;

    // Update the message ID to `local_id` and send a delete request to the daemon.
    msg.id_num = local_id;
    send_delete(daemon_port, msg, socket, res)?;

    // Update the message ID to `remote_id` and send a delete request to the daemon.
    msg.id_num = remote_id;
    send_delete(daemon_port, msg, socket, res)?;

    remove_file("invite_ids.txt").map_err(TalkError::IoError)?;

    let write_stream = stream.try_clone().map_err(TalkError::IoError)?;
    let read_stream = stream.try_clone().map_err(TalkError::IoError)?;

    let top_line = Arc::new(Mutex::new(2));
    let bottom_line = Arc::new(Mutex::new(0));

    // Spawn a thread to handle incoming data from the TCP read stream and update the terminal accordingly.
    spawn_input_thread(
        read_stream,
        height / 2,               // Set split row at half of the terminal height
        width,                    // Set terminal width
        Arc::clone(&top_line),    // Clone the top line reference for thread-safe use
        Arc::clone(&bottom_line), // Clone the bottom line reference for thread-safe use
    )?;

    // Handle user input from stdin, writing it to the TCP write stream and updating the terminal's top line.
    handle_stdin_input(write_stream, height / 2, Arc::clone(&top_line))
        .map_err(TalkError::IoError)?;

    Ok(())
}
fn spawn_input_thread(
    read_stream: TcpStream,
    split_row: u16,
    width: u16,
    top_line: Arc<Mutex<u16>>,
    bottom_line: Arc<Mutex<u16>>,
) -> Result<(), TalkError> {
    thread::spawn(move || {
        // Initialize terminal drawing
        let mut handle = match draw_terminal(split_row, width) {
            Ok(handle) => handle,
            Err(e) => {
                eprintln!("Failed to draw terminal: {}", e);
                return;
            }
        };

        let mut buffer = [0; 128];
        let mut stream = read_stream;

        loop {
            match stream.read(&mut buffer) {
                Ok(nbytes) => {
                    if nbytes > 0 {
                        // Convert buffer data to UTF-8 and process input
                        let input = match std::str::from_utf8(&buffer[..nbytes]) {
                            Ok(input) => input,
                            Err(e) => {
                                eprintln!("Failed to convert buffer to UTF-8 string: {}", e);
                                continue;
                            }
                        };

                        // Call handle_user_input to display the input on the terminal
                        if let Err(e) = handle_user_input(
                            &mut handle,
                            input,
                            split_row,
                            Arc::clone(&top_line),
                            Arc::clone(&bottom_line),
                        ) {
                            eprintln!("Failed to handle user input: {}", e);
                            continue;
                        }
                    } else {
                        // Handle connection closure if no bytes are received
                        handle_connection_close();
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("Error reading from stream: {}", e);
                    break;
                }
            }
        }
    });

    Ok(())
}

fn write_message(
    write_stream: &mut TcpStream,
    message: String,
    top_line: Arc<Mutex<u16>>,
    split_row: u16,
) -> Result<(), io::Error> {
    let mut top_line = top_line.lock().unwrap();
    *top_line += 1;
    write_stream.write_all(message.as_bytes())?;
    write_stream.write_all(b"\n")?;

    if *top_line >= split_row.checked_sub(1).unwrap_or(0) {
        eprint!("\x1B[{};H", 2);
        *top_line = 2;
    }

    Ok(())
}
fn handle_stdin_input(
    mut write_stream: TcpStream,
    split_row: u16,
    top_line: Arc<Mutex<u16>>,
) -> Result<(), io::Error> {
    let stdin = io::stdin();
    let handle = stdin.lock();
    let mut input_buffer = String::new();
    for line in handle.lines() {
        match line {
            Ok(message) => {
                input_buffer.push_str(&message);

                if input_buffer.len() > MAX_USER_INPUT_LENGTH {
                    eprintln!("Warning: You are inputting a large amount of data!");
                    input_buffer.truncate(MAX_USER_INPUT_LENGTH);
                }

                write_message(
                    &mut write_stream,
                    input_buffer.clone(),
                    Arc::clone(&top_line),
                    split_row,
                )?;

                input_buffer.clear();
            }
            Err(e) => {
                eprintln!("Failed to read from stdin: {}", e);
                break;
            }
        }
    }
    Ok(())
}

fn handle_new_invitation(
    daemon_port: u16,
    msg: &mut CtlMsg,
    socket: &UdpSocket,
    res: &mut CtlRes,
    my_machine_addr: Ipv4Addr,
    logger: &mut StateLogger,
) -> Result<(), TalkError> {
    let (socket_addr, listener) = open_sockt(my_machine_addr).map_err(TalkError::IoError)?;

    logger.set_state("[Service connection established.]");

    // Create the socket address data and set it in the `msg`.
    let tcp_data = msg.create_sockaddr_data(&socket_addr.ip().to_string(), socket_addr.port());
    msg.addr.sa_data = tcp_data;

    logger.set_state("[Waiting for your party to respond]");

    // Send the announce message to the daemon, informing it of the new invitation.
    announce(daemon_port, msg, socket, res)?;
    let remote_id = res.id_num;

    // Send the leave invitation message to clear the previous invite state.
    leave_invite(daemon_port, msg, socket, res)?;
    let local_id = res.id_num;

    save_invite_ids_to_file(local_id, remote_id).map_err(TalkError::IoError)?;

    // Start listening for incoming TCP connections.
    for stream in listener.incoming() {
        match stream {
            Ok(client_stream) => {
                if let Err(e) = handle_client(client_stream) {
                    eprintln!("Failed to handle client: {}", e);
                }
            }
            Err(e) => eprintln!("Failed to accept connection: {}", e),
        }
    }

    Ok(())
}

// Retrieves the current user's login name.
fn get_current_user_name() -> Result<String, io::Error> {
    unsafe {
        let login_name = libc::getlogin();
        if !login_name.is_null() {
            Ok(CStr::from_ptr(login_name).to_string_lossy().into_owned())
        } else {
            let pw = getpwuid(getuid());
            // If no user information is found, return an error.
            if pw.is_null() {
                Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "You don't exist. Go away.",
                ))
            } else {
                // Convert the pw_name (user name) from the passwd struct to a Rust String.
                Ok(CStr::from_ptr((*pw).pw_name).to_string_lossy().into_owned())
            }
        }
    }
}

// Retrieves the local machine's hostname.
fn get_local_machine_name() -> Result<String, io::Error> {
    let mut buffer = vec![0 as c_char; HOSTNAME_BUFFER_SIZE];
    let result = unsafe { gethostname(buffer.as_mut_ptr(), buffer.len()) };

    if result == 0 {
        let c_str = unsafe { CStr::from_ptr(buffer.as_ptr()) };
        Ok(c_str.to_string_lossy().into_owned())
    } else {
        Err(Error::new(
            io::ErrorKind::Other,
            "Cannot get local hostname",
        ))
    }
}

// Parses a user-provided address, which could be in the form "user@host" or "host:user".
// If no delimiter is found in the address, it returns the address as the user and the local machine name as the host.
fn parse_address(address: &str, my_machine_name: &str) -> (String, String) {
    // Look for the first occurrence of any delimiter character ('@', ':', '!', '.').
    let at_index = address.find(|c| "@:!.".contains(c));

    // If a delimiter is found, determine how to split the address into user and host.
    if let Some(index) = at_index {
        let delimiter = address.chars().nth(index);

        match delimiter {
            // If the delimiter is '@', split the address as "user@host".
            Some('@') => {
                let (user, host) = address.split_at(index);
                // Extract the host by skipping the '@' character.
                let host = host.get(1..).unwrap_or_default();
                (user.to_string(), host.to_string())
            }
            // For any other delimiter, split the address as "host:user".
            _ => {
                let (host, user) = address.split_at(index);
                // Extract the user by skipping the delimiter character.
                let user = user.get(1..).unwrap_or_default();
                (user.to_string(), host.to_string())
            }
        }
    } else {
        (address.to_string(), my_machine_name.to_string())
    }
}

// Retrieves and sets the names for both local and remote users, then updates the control message.
fn get_names(
    msg: &mut CtlMsg,
    address: &str,
    ttyname: Option<String>,
) -> Result<(String, String), io::Error> {
    let my_name = get_current_user_name()?;
    let my_machine_name = get_local_machine_name()?;

    let (his_name, his_machine_name) = parse_address(address, &my_machine_name);
    msg.vers = TALK_VERSION;
    msg.addr.sa_family = AF_INET as SaFamily;
    msg.ctl_addr.sa_family = AF_INET as SaFamily;
    msg.l_name = string_to_c_string(&my_name);
    msg.r_name = string_to_c_string(&his_name);
    msg.r_tty = tty_to_c_string(&ttyname.unwrap_or_default());

    Ok((my_machine_name, his_machine_name))
}

// Converts a Rust string to a C-style string and stores it in a fixed-size buffer.
fn string_to_c_string(s: &str) -> [c_char; BUFFER_SIZE] {
    let mut buffer: [c_char; BUFFER_SIZE] = [0; BUFFER_SIZE];
    let c_string = CString::new(s).expect("CString::new failed");
    let bytes = c_string.to_bytes();

    // Copy the bytes into the buffer, leaving space for the null terminator.
    for (i, &byte) in bytes.iter().take(BUFFER_SIZE - 1).enumerate() {
        buffer[i] = byte as c_char;
    }
    buffer
}

// Converts a Rust string to a C-style string for terminal names.
fn tty_to_c_string(s: &str) -> [c_char; 16] {
    let mut buffer: [c_char; 16] = [0; 16];
    let c_string = CString::new(s).expect("CString::new failed");
    let bytes = c_string.to_bytes();

    // Copy the bytes into the buffer, leaving space for the null terminator.
    for (i, &byte) in bytes.iter().take(16 - 1).enumerate() {
        buffer[i] = byte as c_char;
    }
    buffer
}

// Resolves the IP addresses for both local and remote machines, and retrieves the service port.
fn get_addrs(
    msg: &mut CtlMsg,
    my_machine_name: &str,
    his_machine_name: &str,
) -> Result<(Ipv4Addr, Ipv4Addr, u16), std::io::Error> {
    let lhost = CString::new(my_machine_name)?;
    let rhost = CString::new(his_machine_name)?;
    let service = CString::new("ntalk")?;
    let protocol = CString::new("udp")?;

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

    let my_machine_addr = resolve_address(&lhost, &hints, my_machine_name)?;

    // If the remote machine is different from the local one, resolve its IP address as well.
    let his_machine_addr = if rhost != lhost {
        resolve_address(&rhost, &hints, his_machine_name)?
    } else {
        my_machine_addr
    };

    // Retrieve the service port for the "ntalk" service using the UDP protocol.
    let daemon_port = get_service_port(&service, &protocol)?;

    Ok((my_machine_addr, his_machine_addr, daemon_port))
}

// Resolves the IP address for the given host.
fn resolve_address(
    host: &CString,
    hints: &addrinfo,
    host_name: &str,
) -> Result<Ipv4Addr, io::Error> {
    let mut res: *mut addrinfo = ptr::null_mut();
    let err = unsafe { getaddrinfo(host.as_ptr(), ptr::null(), hints, &mut res) };

    if err != 0 {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "Error resolving address for {}: {}",
                host_name,
                unsafe { CStr::from_ptr(libc::gai_strerror(err)) }.to_string_lossy()
            ),
        ));
    }

    let mut addr = Ipv4Addr::UNSPECIFIED;
    let mut ai = res;
    while !ai.is_null() {
        let ai_ref = unsafe { &*ai };
        if ai_ref.ai_family == AF_INET {
            let sockaddr: &sockaddr_in = unsafe { &*(ai_ref.ai_addr as *const sockaddr_in) };
            addr = Ipv4Addr::from(u32::from_be(sockaddr.sin_addr.s_addr));
            break;
        }
        ai = ai_ref.ai_next;
    }

    if addr == Ipv4Addr::UNSPECIFIED {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Address not found for {}", host_name),
        ));
    }

    Ok(addr)
}

fn is_service_running(service_name: &str) -> bool {
    let proc_dir = "/proc";

    // Read the contents of the /proc directory to find running processes
    if let Ok(entries) = std::fs::read_dir(proc_dir) {
        for entry in entries {
            if let Ok(entry) = entry {
                // Each entry in /proc is a directory named after the PID of the process
                if let Ok(_pid) = entry.file_name().to_string_lossy().parse::<u32>() {
                    let cmdline_path = format!("{}/cmdline", entry.path().display());
                    // Try to read the command line used to launch the process
                    if let Ok(cmdline) = std::fs::read_to_string(cmdline_path) {
                        if cmdline.contains(service_name) {
                            return true; // The service is running if found in cmdline
                        }
                    }
                }
            }
        }
    }

    false // Service not found in running processes
}

fn get_service_port(service: &CString, protocol: &CString) -> Result<u16, io::Error> {
    // Get the service by name
    let talkd_service = unsafe { getservbyname(service.as_ptr(), protocol.as_ptr()) };

    // Check if the service was found
    if talkd_service.is_null() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "Service {} with protocol {} not registered.",
                service.to_str().unwrap_or("invalid service"),
                protocol.to_str().unwrap_or("invalid protocol")
            ),
        ));
    }

    if is_service_running("talk.socket") {
        // Safely get the port number from the service
        let port = unsafe { (*talkd_service).s_port };
        Ok(u16::from_be(port as u16))
    } else {
        Ok(2222)
    }
}

// Handles the client's connection by spawning a read thread and handling user input.
fn handle_client(stream: TcpStream) -> Result<(), io::Error> {
    let write_stream = stream.try_clone()?;

    let (width, height) = get_terminal_size();
    let split_row = height / 2;
    let top_line = Arc::new(Mutex::new(2));
    let bottom_line = Arc::new(Mutex::new(0));

    let top_line_clone = Arc::clone(&top_line);
    let bottom_line_clone = Arc::clone(&bottom_line);

    // Spawn a separate thread to handle reading from the stream.
    spawn_read_thread(stream, split_row, width, top_line_clone, bottom_line_clone)?;
    // Handle user input from stdin and send it to the write stream.
    handle_stdin_input(write_stream, split_row, top_line)?;

    Ok(())
}

// Spawns a new thread to read data from the TCP stream and display it on the terminal.
fn spawn_read_thread(
    stream: TcpStream,
    split_row: u16,
    width: u16,
    top_line: Arc<Mutex<u16>>,
    bottom_line: Arc<Mutex<u16>>,
) -> Result<(), io::Error> {
    thread::spawn(move || {
        // Attempt to draw the terminal and handle any errors
        let mut handle = match draw_terminal(split_row, width) {
            Ok(handle) => handle,
            Err(e) => {
                eprintln!("Failed to draw terminal: {}", e);
                return;
            }
        };

        let mut buffer = [0; 128];
        let mut stream = stream;

        loop {
            match stream.read(&mut buffer) {
                Ok(nbytes) => {
                    if nbytes > 0 {
                        // Handle user input and manage potential errors
                        if let Err(e) = handle_user_input(
                            &mut handle,
                            std::str::from_utf8(&buffer[..nbytes]).unwrap_or(""),
                            split_row,
                            Arc::clone(&top_line),
                            Arc::clone(&bottom_line),
                        ) {
                            eprintln!("Error handling user input: {}", e);
                            break;
                        }
                    } else {
                        // Handle connection closure
                        handle_connection_close();
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("Error reading from stream: {}", e);
                    break;
                }
            }
        }
    });

    Ok(())
}

// Handles user input and displays it in the bottom section of the terminal.
fn handle_user_input(
    handle: &mut io::StdoutLock,
    input: &str,
    split_row: u16,
    top_line: Arc<Mutex<u16>>,
    bottom_line: Arc<Mutex<u16>>,
) -> io::Result<()> {
    let top_line = top_line.lock().unwrap();
    let mut bottom_line = bottom_line.lock().unwrap();

    // Reset bottom line position if it exceeds the available space.
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

// Draws the terminal interface with a split line separating the top and bottom sections.
fn draw_terminal(split_row: u16, width: u16) -> io::Result<io::StdoutLock<'static>> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();

    // Clear terminal screen
    print!("\x1B[2J\x1B[H");
    io::stdout().flush().ok();

    // Display the connection message at the top of the terminal.
    write!(handle, "[Connection established]")?;
    // Move the cursor to the split row and draw the split line.
    write!(handle, "\x1b[{};0H", split_row)?;
    // Draw the horizontal split line (─) across the width of the terminal.
    writeln!(
        handle,
        "└{:─<width$}┘",
        "",
        width = (width as usize).checked_sub(2).unwrap_or(0)
    )?;
    // Move the cursor back to the top-left corner and then down by one line.
    write!(handle, "\x1b[1;H")?;
    write!(handle, "\x1B[1B")?;

    handle.flush()?;

    Ok(handle)
}

// Opens a TCP socket bound to the provided IPv4 address.
// Used for talk users sending and receiving messages.
fn open_sockt(my_machine_addr: Ipv4Addr) -> Result<(SocketAddr, TcpListener), io::Error> {
    let listener = TcpListener::bind((my_machine_addr, 0))?;
    let addr = listener.local_addr()?;

    Ok((addr, listener))
}

// Opens a UDP socket bound to the provided IPv4 address.
// Used for checking properly access to connection.
fn open_ctl(my_machine_addr: Ipv4Addr) -> Result<(SocketAddr, UdpSocket), io::Error> {
    let socket = UdpSocket::bind((my_machine_addr, 0))?;
    let addr = socket.local_addr()?;

    Ok((addr, socket))
}

fn handle_connection_close() {
    //clear terminal screen
    print!("\x1B[2J\x1B[H");
    io::stdout().flush().ok();
    eprintln!("Connection closed, exiting...");

    process::exit(3);
}

// Handles sending a message (CtlMsg) to the daemon and receiving a response.
// Uses the 'reqwest' function to handle the communication.
fn handle_invite(
    daemon_port: u16,
    msg: &mut CtlMsg,
    socket: &UdpSocket,
    res: &mut CtlRes,
    msg_type: MessageType,
) -> Result<(), TalkError> {
    reqwest(daemon_port, msg, msg_type, socket, res)
}

// Looks for an invite by sending a LookUp message to the daemon.
fn look_for_invite(
    daemon_port: u16,
    msg: &mut CtlMsg,
    socket: &UdpSocket,
    res: &mut CtlRes,
) -> Result<(), TalkError> {
    handle_invite(daemon_port, msg, socket, res, MessageType::LookUp)
}

// Leaves an invite by sending a LeaveInvite message to the daemon.
fn leave_invite(
    daemon_port: u16,
    msg: &mut CtlMsg,
    socket: &UdpSocket,
    res: &mut CtlRes,
) -> Result<(), TalkError> {
    handle_invite(daemon_port, msg, socket, res, MessageType::LeaveInvite)
}

// Announces an invite by sending an Announce message to the daemon.
fn announce(
    daemon_port: u16,
    msg: &mut CtlMsg,
    socket: &UdpSocket,
    res: &mut CtlRes,
) -> Result<(), TalkError> {
    handle_invite(daemon_port, msg, socket, res, MessageType::Announce)
}

// Sends a delete message to the daemon.
fn send_delete(
    daemon_port: u16,
    msg: &mut CtlMsg,
    socket: &UdpSocket,
    res: &mut CtlRes,
) -> Result<(), TalkError> {
    handle_invite(daemon_port, msg, socket, res, MessageType::Delete)
}

fn save_invite_ids_to_file(local_id: u32, remote_id: u32) -> io::Result<()> {
    let mut file = File::create("invite_ids.txt")?;
    writeln!(file, "local_id={}", local_id)?;
    writeln!(file, "remote_id={}", remote_id)?;
    Ok(())
}

// Sends a message (CtlMsg) to the talk daemon over a UDP socket and waits for a response.
// The function will retry sending if it encounters a WouldBlock error.
// The response is parsed into a CtlRes struct.
fn reqwest(
    daemon_port: u16,      // Port number of the talk daemon
    msg: &mut CtlMsg,      // The control message (CtlMsg) to be sent
    msg_type: MessageType, // Type of the message (used to set the message type)
    socket: &UdpSocket,    // UDP socket to send and receive messages
    res: &mut CtlRes,      // Reference to store the received response (CtlRes)
) -> Result<(), TalkError> {
    let talkd_addr: SocketAddr = format!("127.0.0.1:{}", 2222)
        .parse()
        .map_err(|e: AddrParseError| TalkError::AddressResolutionFailed(e.to_string()))?;

    msg.r#type = msg_type as u8;
    let msg_bytes = msg
        .to_bytes()
        .map_err(|e| TalkError::Other(e.to_string()))?;

    loop {
        // Try sending the message bytes to the talk daemon
        match socket.send_to(&msg_bytes, talkd_addr) {
            Ok(_) => {
                let mut buf = [0; 1024];
                // Try to receive a response from the talk daemon
                match socket.recv_from(&mut buf) {
                    Ok((amt, _)) => {
                        let ctl_res = CtlRes::from_bytes(&buf[..amt])
                            .map_err(|e| TalkError::Other(e.to_string()))?;
                        *res = ctl_res;
                        break;
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                    Err(e) => {
                        eprintln!("Error receiving message: {}", e);
                        return Err(TalkError::IoError(e));
                    }
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_secs(1));
            }
            Err(e) => {
                eprintln!("Error sending message: {}", e);
                return Err(TalkError::IoError(e));
            }
        }
    }
    Ok(())
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

// Registers signal handlers for specific signals (SIGINT, SIGQUIT, SIGPIPE).
// The signal handler function is `handle_signals`.
pub fn register_signals() {
    unsafe {
        let signals = &[SIGINT, SIGQUIT, SIGPIPE];
        for &sig in signals {
            if signal(sig, handle_signals as usize) == libc::SIG_ERR {
                eprintln!("Failed to register signal handler for signal {}", sig);
            }
        }
    }
}

/// Handles incoming signals by setting the interrupt flag and exiting the process.
pub fn handle_signals(signal_code: libc::c_int) {
    //clear terminal screen
    eprint!("\x1B[2J\x1B[H");

    eprintln!("Connection closed, exiting...");

    std::process::exit(128 + signal_code);
}

// Retrieves the terminal size (width and height in characters) using an ioctl system call.
fn get_terminal_size() -> (u16, u16) {
    let mut size: winsize = unsafe { zeroed() };

    unsafe {
        if ioctl(STDOUT_FILENO, TIOCGWINSZ, &mut size) == -1 {
            eprintln!("Failed to get terminal size");
            // Default fallback size
            return (80, 24);
        }
    }

    (size.ws_col, size.ws_row)
}
