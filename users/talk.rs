// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

// todo:
// - change timeout to re-connect in send_to
// - check remote connection (in progress)

extern crate clap;
extern crate gettextrs;
extern crate plib;

use clap::{error::ErrorKind, Parser};
use gettextrs::{bind_textdomain_codeset, setlocale, textdomain, LocaleCategory};
use plib::PROJECT_NAME;
use thiserror::Error;

#[cfg(target_os = "linux")]
use libc::sa_family_t;
use libc::{
    addrinfo, c_char, c_uchar, getaddrinfo, gethostname, getpid, getpwuid, getservbyname, getuid,
    ioctl, signal, sockaddr_in, winsize, AF_INET, AI_CANONNAME, SIGINT, SIGPIPE, SIGQUIT,
    SOCK_DGRAM, STDOUT_FILENO, TIOCGWINSZ,
};
use std::{
    ffi::{CStr, CString},
    io::{self, BufRead, Cursor, Error, Read, Write},
    mem::{size_of, zeroed},
    net::{
        self, AddrParseError, Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream, UdpSocket,
    },
    process, ptr,
    sync::{Arc, LazyLock, Mutex},
    thread,
    time::{Duration, Instant},
};

/// A static variable to hold the state of delete invitations on SIGINT signal.
static DELETE_INVITATIONS: LazyLock<Arc<Mutex<Option<(Vec<u8>, Arc<UdpSocket>, SocketAddr)>>>> =
    LazyLock::new(|| Arc::new(Mutex::new(None)));

/// The size of the buffer for control message fields like l_name, r_name, and r_tty in CtlMsg.
const BUFFER_SIZE: usize = 12;

//i The maximum size for the buffer to store the hostname, based on typical hostname lengths.
const HOSTNAME_BUFFER_SIZE: usize = 256;

/// The maximum number of characters allowed for user input in a single operation.
const MAX_USER_INPUT_LENGTH: usize = 128;
/// The version number for the talk protocol.
const TALK_VERSION: u8 = 1;

#[derive(Debug, Copy, Clone, PartialEq)]
/// Represents the types of messages exchanged in the communication.
enum MessageType {
    /// Leave invitation with server.
    LeaveInvite,
    /// Check for invitation by callee.
    LookUp,
    /// Delete invitation by caller.
    Delete,
    /// Announce invitation by caller.
    Announce,
}

impl TryFrom<u8> for MessageType {
    type Error = TalkError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(MessageType::LeaveInvite),
            1 => Ok(MessageType::LookUp),
            2 => Ok(MessageType::Delete),
            3 => Ok(MessageType::Announce),
            _ => Err(TalkError::Other(
                "Not existing MessageType provided".to_string(),
            )),
        }
    }
}

#[derive(Debug, PartialEq)]
/// Represents the possible responses from a request.
enum Answer {
    /// Operation completed properly.
    Success,
    /// Callee not logged in.
    NotHere,
    /// Operation failed for unexplained reason.
    Failed,
    /// Caller’s machine name is unknown.
    MachineUnknown,
    /// Callee’s TTY doesn’t permit announce.
    PermissionDenied,
    /// Request has an invalid type value.
    UnknownRequest,
    /// Request has an invalid protocol version.
    BadVersion,
    /// Request has an invalid address value.
    BadAddr,
    /// Request has an invalid control address value.
    BadCtlAddr,
}

impl TryFrom<u8> for Answer {
    type Error = TalkError;

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
            _ => Err(TalkError::Other(
                "Not existingi Answer provided".to_string(),
            )),
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

#[derive(Debug, Error)]
pub enum TalkError {
    #[error("Usage: talk user [ttyname]")]
    InvalidArguments,
    #[error("Not a TTY")]
    NotTty,
    #[error("Failed to resolve addresses: {0}")]
    AddressResolutionFailed(String),
    #[error("I/O error: {0}")]
    IoError(#[from] io::Error),
    #[error("An error occurred: {0}")]
    Other(String),
}

#[cfg(target_os = "macos")]
type SaFamily = u16;

#[cfg(target_os = "linux")]
type SaFamily = sa_family_t;

#[repr(C, packed)]
/// Socket address structure representing a network address.
pub struct Osockaddr {
    /// Address family (e.g., IPv4, IPv6).
    pub sa_family: SaFamily,
    /// Address data, including the port and IP address.
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

impl Default for Osockaddr {
    fn default() -> Self {
        Osockaddr {
            sa_family: 0,
            sa_data: [0; 14],
        }
    }
}

#[repr(C, packed)]
/// Control message structure used for communication in the talk protocol.
struct CtlMsg {
    /// Version of the message.
    vers: c_uchar,
    /// Type of the message.
    r#type: c_uchar,
    /// Answer code (success or failure).
    answer: c_uchar,
    /// Padding for alignment.
    pad: c_uchar,
    /// Identifier number for the message.
    id_num: u32,
    /// Socket address of the recipient.
    addr: Osockaddr,
    /// Control socket address.
    ctl_addr: Osockaddr,
    /// Process ID of the sender.
    pid: i32,
    /// Local user name.
    l_name: [c_char; 12],
    /// Remote user name.
    r_name: [c_char; 12],
    /// Remote terminal name.
    r_tty: [c_char; 16],
}

impl Default for CtlMsg {
    fn default() -> Self {
        CtlMsg {
            vers: 1,
            r#type: MessageType::LookUp as u8,
            answer: Answer::Success as u8,
            pad: 0,
            id_num: 0,
            addr: Osockaddr::default(),
            ctl_addr: Osockaddr::default(),
            pid: 0,
            l_name: string_to_c_string(""),
            r_name: string_to_c_string(""),
            r_tty: [0; 16],
        }
    }
}

impl CtlMsg {
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
/// Control response structure used for communication with the daemon.
pub struct CtlRes {
    /// Version of the control protocol.
    pub vers: c_uchar,

    /// Type of message being sent/received.
    r#type: MessageType,

    /// Response to the control message.
    answer: Answer,

    /// Padding byte to maintain alignment.
    pub pad: c_uchar,

    /// Unique identifier number for the invitation.
    pub id_num: u32,

    /// Socket address associated with the response.
    pub addr: Osockaddr,
}

impl Default for CtlRes {
    fn default() -> Self {
        CtlRes {
            vers: 0,
            r#type: MessageType::LookUp,
            answer: Answer::UnknownRequest,
            pad: 0,
            id_num: 0,
            addr: Osockaddr {
                sa_family: 0,
                sa_data: [0; 14],
            },
        }
    }
}

impl CtlRes {
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
    address: String,

    /// Terminal name to use (optional)
    ttyname: Option<String>,
}

fn talk(args: Args) -> Result<(), TalkError> {
    check_if_tty()?;

    let mut msg = CtlMsg::default();
    let mut res = CtlRes::default();
    let (width, height) = get_terminal_size();

    let mut logger = StateLogger::new("No connection yet.");

    // Retrieve the local and remote machine names
    let (my_machine_name, his_machine_name) =
        get_names(&mut msg, &args.address, args.ttyname).map_err(|e| TalkError::IoError(e))?;

    // Get the local and remote addresses, and the daemon port number
    let (my_machine_addr, his_machine_addr, daemon_port) =
        get_addrs(&mut msg, &my_machine_name, &his_machine_name)
            .map_err(|e| TalkError::IoError(e))?;

    // Open control socket
    let (ctl_addr, socket) = open_ctl(my_machine_addr).map_err(|e| TalkError::IoError(e))?;

    let ctl_addr_data = msg.create_ctl_addr(ctl_addr);
    msg.ctl_addr.sa_data = ctl_addr_data;

    logger.set_state("[Checking for invitation on caller's machine]");

    let talkd_addr: SocketAddr = format!("{}:{}", his_machine_addr, daemon_port)
        .parse()
        .map_err(|e: AddrParseError| TalkError::AddressResolutionFailed(e.to_string()))?;

    // Look for an invitation from the daemon
    look_for_invite(daemon_port, his_machine_addr, &mut msg, &socket, &mut res)?;
    // msg.id_num = res.id_num.to_be();
    // send_delete(daemon_port, his_machine_addr, &mut msg, &socket, &mut res)?;

    // Set the invitation ID number and send a delete request for the old invitation
    if res.answer == Answer::Success {
        handle_existing_invitation(width, height, &mut res)?;
    } else if res.answer == Answer::UnknownRequest {
        let msg_bytes = msg
            .to_bytes()
            .map_err(|e| TalkError::Other(e.to_string()))?;

        let talkd_addr: SocketAddr = format!("0.0.0.0:{}", 8081)
            .parse()
            .map_err(|e: AddrParseError| TalkError::AddressResolutionFailed(e.to_string()))?;

        socket
            .send_to(&msg_bytes, talkd_addr)
            .map_err(|e| TalkError::IoError(e))?;
    } else {
        logger.set_state("[Waiting to connect with caller]");
        handle_new_invitation(
            talkd_addr,
            daemon_port,
            his_machine_addr,
            &mut msg,
            socket,
            &mut res,
            my_machine_addr,
            &mut logger,
        )?;
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::try_parse().unwrap_or_else(|err| {
        if err.kind() == ErrorKind::DisplayHelp || err.kind() == ErrorKind::DisplayVersion {
            // Print help or version message
            eprintln!("{}", err);
        } else {
            // Print custom error message
            eprintln!("Error parsing arguments: {}", err);
        }

        // Exit with a non-zero status code
        std::process::exit(1);
    });
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

/// Checks if the standard input is a TTY (terminal).
///
/// # Returns
///
/// A `Result` indicating success or a `TalkError` if the input is not a TTY.
fn check_if_tty() -> Result<(), TalkError> {
    if atty::isnt(atty::Stream::Stdin) {
        eprintln!("Not a TTY");
        return Err(TalkError::NotTty);
    }
    Ok(())
}

/// Handles an existing invitation by establishing a TCP connection and managing user input.
///
/// # Arguments
///
/// * `width` - The width of the terminal for drawing purposes.
/// * `height` - The height of the terminal for positioning.
/// * `res` - A mutable reference to the control response containing the address.
///
/// # Returns
///
/// A `Result` indicating success or a `TalkError`.
fn handle_existing_invitation(width: u16, height: u16, res: &mut CtlRes) -> Result<(), TalkError> {
    let tcp_addr = res.addr.to_socketaddr().ok_or_else(|| {
        TalkError::AddressResolutionFailed("Failed to convert address to socket address.".into())
    })?;

    // Establish a TCP connection to the `tcp_addr`. Map any IO errors to `TalkError::IoError`.
    let stream = TcpStream::connect(tcp_addr).map_err(TalkError::IoError)?;

    let write_stream = stream.try_clone().map_err(TalkError::IoError)?;
    let read_stream = stream.try_clone().map_err(TalkError::IoError)?;

    let top_line = Arc::new(Mutex::new(2));
    let bottom_line = Arc::new(Mutex::new(0));

    // Spawn a thread to handle incoming data from the TCP read stream and update the terminal accordingly.
    spawn_input_thread(
        read_stream,
        height / 2,
        width,
        Arc::clone(&top_line),
        Arc::clone(&bottom_line),
    )?;

    // Handle user input from stdin, writing it to the TCP write stream and updating the terminal's top line.
    handle_stdin_input(write_stream, height / 2, Arc::clone(&top_line))
        .map_err(TalkError::IoError)?;

    Ok(())
}

/// Spawns a thread to handle input from a TCP stream and update the terminal interface.
///
/// # Arguments
///
/// * `read_stream` - The TCP stream to read input from.
/// * `split_row` - The row in the terminal where the split occurs.
/// * `width` - The width of the terminal for drawing purposes.
/// * `top_line` - An `Arc` of a `Mutex` that tracks the top line position in the terminal.
/// * `bottom_line` - An `Arc` of a `Mutex` that tracks the bottom line position in the terminal.
///
/// # Returns
///
/// A `Result` indicating success or a `TalkError`.
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

/// Writes a message to a TCP stream and updates the top line position in the terminal.
///
/// # Arguments
///
/// * `write_stream` - A mutable reference to the TCP stream where the message will be sent.
/// * `message` - The message string to be sent.
/// * `top_line` - An `Arc` of a `Mutex` that tracks the top line position in the terminal.
/// * `split_row` - The row in the terminal where the split occurs.
///
/// # Returns
///
/// A `Result` indicating success or an `io::Error`.
fn write_message(
    write_stream: &mut TcpStream,
    message: String,
    top_line: Arc<Mutex<u16>>,
    split_row: u16,
) -> Result<(), io::Error> {
    let mut top_line = top_line.lock().unwrap();
    *top_line += 1;

    // Write the message to the TCP stream
    write_stream.write_all(message.as_bytes())?;
    write_stream.write_all(b"\n")?;

    if *top_line >= split_row.checked_sub(1).unwrap_or(0) {
        eprint!("\x1B[{};H", 2);
        *top_line = 2;
    }

    Ok(())
}
/// Handles user input from stdin, sending it over a TCP stream.
///
/// # Arguments
///
/// * `write_stream` - The TCP stream to send user input to.
/// * `split_row` - The row in the terminal where the input area begins.
/// * `top_line` - An `Arc` of a `Mutex` that tracks the top line position in the terminal.
///
/// # Returns
///
/// A `Result` indicating success or an `io::Error`.
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

/// Handles a new invitation by setting up a TCP socket, notifying the daemon,
/// and managing incoming connections.
///
/// # Arguments
///
/// * `talkd_addr` - The socket address of the talk daemon.
/// * `daemon_port` - The port number for the talk daemon.
/// * `his_machine_addr` - The IPv4 address of the remote machine.
/// * `msg` - A mutable reference to the control message (`CtlMsg`).
/// * `socket` - An `Arc` of the UDP socket used for communication.
/// * `res` - A mutable reference to the control response (`CtlRes`).
/// * `my_machine_addr` - The IPv4 address of the local machine.
/// * `logger` - A mutable reference to the state logger for logging state changes.
///
/// # Returns
///
/// A `Result` indicating success or a `TalkError`.
fn handle_new_invitation(
    talkd_addr: SocketAddr,
    daemon_port: u16,
    his_machine_addr: Ipv4Addr,
    msg: &mut CtlMsg,
    socket: Arc<UdpSocket>,
    res: &mut CtlRes,
    my_machine_addr: Ipv4Addr,
    logger: &mut StateLogger,
) -> Result<(), TalkError> {
    let (socket_addr, listener) = open_sockt(my_machine_addr).map_err(TalkError::IoError)?;

    logger.set_state("[Service connection established.]");

    // Create the socket address data and set it in the `msg`.
    let tcp_data = msg.create_sockaddr_data(&socket_addr.ip().to_string(), socket_addr.port());

    logger.set_state("[Waiting for your party to respond]");

    msg.addr.sa_data = tcp_data;

    // Send the leave invitation message to clear the previous invite state.
    leave_invite(daemon_port, my_machine_addr, msg, &socket, res)?;
    let local_id = res.id_num.to_be();
    msg.r#type = MessageType::Delete as u8;
    msg.id_num = local_id;

    // Send the announce message to the daemon, informing it of the new invitation.
    announce(daemon_port, his_machine_addr, msg, &socket, res)?;
    let remote_id = res.id_num.to_be();

    send_delete(daemon_port, his_machine_addr, msg, &socket, res)?;
    let local_id = res.id_num.to_be();
    msg.r#type = MessageType::Delete as u8;
    msg.id_num = local_id;

    let msg_bytes = msg
        .to_bytes()
        .map_err(|e| TalkError::Other(e.to_string()))?;

    let clone_socket = Arc::clone(&socket);
    *DELETE_INVITATIONS.lock().unwrap() = Some((msg_bytes, clone_socket, talkd_addr));
    // Start listening for incoming TCP connections.
    for stream in listener.incoming() {
        match stream {
            Ok(client_stream) => {
                msg.id_num = local_id;
                send_delete(daemon_port, his_machine_addr, msg, &socket, res)?;
                msg.id_num = remote_id;
                send_delete(daemon_port, his_machine_addr, msg, &socket, res)?;

                if let Err(e) = handle_client(client_stream) {
                    eprintln!("Failed to handle client: {}", e);
                }
            }
            Err(e) => eprintln!("Failed to accept connection: {}", e),
        }
    }

    Ok(())
}

/// Retrieves the current user's login name.
///
/// # Returns
///
/// A `Result` containing the login name as a `String` on success,
/// or an `io::Error` if the user cannot be found.
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

/// Retrieves the local machine's hostname.
///
/// # Returns
///
/// A `Result` containing the hostname as a `String` on success, or an `io::Error` on failure.
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

/// Parses a user-provided address, which can be in the format "user@host" or "host:user".
/// If no delimiter is found, returns the address as the user and the local machine name as the host.
///
/// # Arguments
///
/// * `address` - A string slice representing the user-provided address.
/// * `my_machine_name` - A string slice representing the local machine name.
///
/// # Returns
///
/// A tuple containing the user name and the host name.
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

/// Retrieves and sets the names for both local and remote users, updating the control message accordingly.
///
/// # Arguments
///
/// * `msg` - A mutable reference to a `CtlMsg` structure that will be updated with user and machine names.
/// * `address` - A string representing the remote user's address.
/// * `ttyname` - An optional string containing the terminal name.
///
/// # Returns
///
/// Returns a tuple containing the local machine name and the remote machine name.
///
/// # Errors
///
/// Returns an error if retrieving user or machine names fails.
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

/// Converts a Rust string to a C-style string and stores it in a fixed-size buffer.
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

/// Converts a Rust string to a C-style string for terminal names.
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

/// Resolves the IP addresses for both the local and remote machines, and retrieves the service port for communication.
///
/// # Arguments
///
/// * `msg` - A mutable reference to a `CtlMsg` structure, which will have its `pid` field set.
/// * `my_machine_name` - A string representing the local machine's hostname.
/// * `his_machine_name` - A string representing the remote machine's hostname.
///
/// # Returns
///
/// Returns a tuple containing the local machine's IP address, the remote machine's IP address, and the service port for the "ntalk" service.
///
/// # Errors
///
/// Returns an error if address resolution or service lookup fails.
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

/// Resolves the IP address for a given host using the specified hints for address resolution.
///
/// # Arguments
///
/// * `host` - A `CString` representing the hostname to resolve.
/// * `hints` - A reference to an `addrinfo` structure providing hints for the resolution process.
/// * `host_name` - A string representing the hostname (for error reporting).
///
/// # Returns
///
/// Returns `Result<Ipv4Addr, io::Error>` containing the resolved IPv4 address on success, or an error if resolution fails.
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

/// Retrieves the port number for a given service and protocol by querying the system's service database.
///
/// # Arguments
///
/// * `service` - A `CString` representing the name of the service (e.g., "talk").
/// * `protocol` - A `CString` representing the protocol (e.g., "udp" or "tcp").
///
/// # Returns
///
/// Returns `Result<u16, io::Error>` containing the port number on success, or an error if the service is not found.
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

    // Safely get the port number from the service
    let port = unsafe { (*talkd_service).s_port };
    Ok(u16::from_be(port as u16))
}

/// Handles the client's connection by spawning a read thread and managing user input from stdin.
///
/// # Arguments
///
/// * `stream` - The `TcpStream` representing the client's connection.
///
/// # Returns
///
/// Returns `Result<(), io::Error>` if the operation completes successfully or an error occurs during execution.
fn handle_client(stream: TcpStream) -> Result<(), io::Error> {
    let write_stream = stream.try_clone()?;

    let (width, height) = get_terminal_size();
    let split_row = height / 2;
    let top_line = Arc::new(Mutex::new(2));
    let bottom_line = Arc::new(Mutex::new(0));

    // Spawn a separate thread to handle reading from the stream.
    spawn_input_thread(
        stream,
        height / 2,
        width,
        Arc::clone(&top_line),
        Arc::clone(&bottom_line),
    )
    .unwrap();

    // Handle user input from stdin and send it to the write stream.
    handle_stdin_input(write_stream, split_row, top_line)?;

    Ok(())
}

/// Handles user input by writing it to the terminal's bottom section and managing the cursor position.
///
/// # Arguments
///
/// * `handle` - A mutable reference to a locked `StdoutLock`, used to write to the terminal.
/// * `input` - The user input string to be displayed.
/// * `split_row` - The row number where the terminal splits between top and bottom sections.
/// * `top_line` - A shared (Arc<Mutex>) reference to the current top line position.
/// * `bottom_line` - A shared (Arc<Mutex>) reference to the current bottom line position.
///
/// # Returns
///
/// Returns `io::Result<()>` if the operation completes successfully.
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

/// Draws the terminal interface with a split line separating the top and bottom sections.
///
/// # Arguments
///
/// * `split_row` - The row number where the split line will be drawn.
/// * `width` - The width of the terminal in characters.
///
/// # Returns
///
/// Returns a `StdoutLock` wrapped in a `Result` if successful.
/// Any `io::Error` encountered during drawing will be returned.
///
/// # Errors
///
/// Returns an `io::Error` if there is an issue with writing to the terminal or flushing output.
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

/// Opens a TCP socket bound to the provided IPv4 address.
/// This socket is used for sending and receiving messages between talk users.
///
/// # Arguments
///
/// * `my_machine_addr` - The IPv4 address of the machine where the socket will be bound.
///
/// # Returns
///
/// Returns a tuple containing the `SocketAddr` and the `TcpListener` on success.
/// Returns an `io::Error` if there is an error in binding the socket.
fn open_sockt(my_machine_addr: Ipv4Addr) -> Result<(SocketAddr, TcpListener), io::Error> {
    let listener = TcpListener::bind((my_machine_addr, 0))?;
    let addr = listener.local_addr()?;

    Ok((addr, listener))
}

/// Opens a UDP socket bound to the provided IPv4 address.
/// The socket is used for checking access to the connection.
///
/// # Arguments
///
/// * `my_machine_addr` - The IPv4 address of the machine where the socket will be bound.
///
/// # Returns
///
/// Returns a tuple containing the `SocketAddr` and a reference-counted `UdpSocket` on success.
/// Returns an `io::Error` if there is an error in binding the socket or setting non-blocking mode.
fn open_ctl(my_machine_addr: Ipv4Addr) -> Result<(SocketAddr, Arc<UdpSocket>), io::Error> {
    let socket = UdpSocket::bind((my_machine_addr, 0))?;
    socket.set_nonblocking(true)?;
    let addr = socket.local_addr()?;

    Ok((addr, Arc::new(socket)))
}

/// Handles sending a message (`CtlMsg`) to the daemon and receiving a response.
/// Internally calls the `request` function for communication.
///
/// # Arguments
///
/// * `daemon_port` - The port number of the talk daemon.
/// * `his_machine_addr` - The IPv4 address of the remote user or localhost.
/// * `msg` - A mutable reference to the control message (`CtlMsg`) to be sent.
/// * `socket` - The UDP socket used to send and receive messages.
/// * `res` - A mutable reference where the received response (`CtlRes`) will be stored.
/// * `msg_type` - The type of message being sent (e.g., Delete, LookUp).
///
fn handle_invite(
    daemon_port: u16,
    his_machine_addr: Ipv4Addr,
    msg: &mut CtlMsg,
    socket: &UdpSocket,
    res: &mut CtlRes,
    msg_type: MessageType,
) -> Result<(), TalkError> {
    request(daemon_port, his_machine_addr, msg, msg_type, socket, res)
}

/// Sends a LookUp message to the talk daemon to search for an invite.
///
/// # Arguments
///
/// * `daemon_port` - The port number of the talk daemon.
/// * `his_machine_addr` - The IPv4 address of the remote user or localhost.
/// * `msg` - A mutable reference to the control message (`CtlMsg`) to be sent.
/// * `socket` - The UDP socket used to send and receive messages.
/// * `res` - A mutable reference where the received response (`CtlRes`) will be stored.
///
fn look_for_invite(
    daemon_port: u16,
    his_machine_addr: Ipv4Addr,
    msg: &mut CtlMsg,
    socket: &UdpSocket,
    res: &mut CtlRes,
) -> Result<(), TalkError> {
    handle_invite(
        daemon_port,
        his_machine_addr,
        msg,
        socket,
        res,
        MessageType::LookUp,
    )
}

/// Sends a LeaveInvite message to the talk daemon, indicating that the user is leaving the invite.
///
/// # Arguments
///
/// * `daemon_port` - The port number of the talk daemon.
/// * `his_machine_addr` - The IPv4 address of the remote user or localhost.
/// * `msg` - A mutable reference to the control message (`CtlMsg`) to be sent.
/// * `socket` - The UDP socket used to send and receive messages.
/// * `res` - A mutable reference where the received response (`CtlRes`) will be stored.
///
fn leave_invite(
    daemon_port: u16,
    his_machine_addr: Ipv4Addr,
    msg: &mut CtlMsg,
    socket: &UdpSocket,
    res: &mut CtlRes,
) -> Result<(), TalkError> {
    handle_invite(
        daemon_port,
        his_machine_addr,
        msg,
        socket,
        res,
        MessageType::LeaveInvite,
    )
}

/// Sends an Announce message to the talk daemon, announcing an invite.
///
/// # Arguments
///
/// * `daemon_port` - The port number of the talk daemon.
/// * `his_machine_addr` - The IPv4 address of the remote user or localhost.
/// * `msg` - A mutable reference to the control message (`CtlMsg`) to be sent.
/// * `socket` - The UDP socket used to send and receive messages.
/// * `res` - A mutable reference where the received response (`CtlRes`) will be stored.
///
fn announce(
    daemon_port: u16,
    his_machine_addr: Ipv4Addr,
    msg: &mut CtlMsg,
    socket: &UdpSocket,
    res: &mut CtlRes,
) -> Result<(), TalkError> {
    handle_invite(
        daemon_port,
        his_machine_addr,
        msg,
        socket,
        res,
        MessageType::Announce,
    )
}

/// Sends a delete request (`CtlMsg`) to the talk daemon over a UDP socket and waits for a response.
/// This function is a wrapper around `handle_invite`, specifically for handling delete messages.
///
/// # Arguments
///
/// * `daemon_port` - The port number of the talk daemon.
/// * `his_machine_addr` - The IPv4 address of the remote user or localhost.
/// * `msg` - A mutable reference to the control message (`CtlMsg`) to be sent.
/// * `socket` - The UDP socket used to send and receive messages.
/// * `res` - A mutable reference where the received response (`CtlRes`) will be stored.
///
fn send_delete(
    daemon_port: u16,
    his_machine_addr: Ipv4Addr,
    msg: &mut CtlMsg,
    socket: &UdpSocket,
    res: &mut CtlRes,
) -> Result<(), TalkError> {
    handle_invite(
        daemon_port,
        his_machine_addr,
        msg,
        socket,
        res,
        MessageType::Delete,
    )
}

/// Sends a control message (`CtlMsg`) to the talk daemon over a UDP socket and waits for a response.
///
/// This function retries sending the message if it encounters a `WouldBlock` error and will continue
/// trying until a response is received or an error occurs.
///
/// # Arguments
///
/// * `daemon_port` - The port number of the talk daemon.
/// * `his_machine_addr` - The IPv4 address of the remote user or localhost.
/// * `msg` - A mutable reference to the control message (`CtlMsg`) to be sent.
/// * `msg_type` - The type of the message, used to set the message type in `msg`.
/// * `socket` - The UDP socket used to send and receive messages.
/// * `res` - A mutable reference where the received response (`CtlRes`) will be stored.
///
/// # Returns
///
/// Returns `Ok(())` if the message is successfully sent and a valid response is received.
/// Returns a `TalkError` if an error occurs during sending, receiving, or message parsing.
///
/// # Errors
///
/// * `TalkError::AddressResolutionFailed` - If the talk daemon's address cannot be resolved.
/// * `TalkError::Other` - If the message serialization or response deserialization fails.
/// * `TalkError::IoError` - If a network I/O error occurs.
fn request(
    daemon_port: u16,
    his_machine_addr: Ipv4Addr,
    msg: &mut CtlMsg,
    msg_type: MessageType,
    socket: &UdpSocket,
    res: &mut CtlRes,
) -> Result<(), TalkError> {
    let talkd_addr: SocketAddr = format!("{}:{}", his_machine_addr, daemon_port)
        .parse()
        .map_err(|e: AddrParseError| TalkError::AddressResolutionFailed(e.to_string()))?;

    msg.r#type = msg_type as u8;
    let msg_bytes = msg
        .to_bytes()
        .map_err(|e| TalkError::Other(e.to_string()))?;

    let start_time = Instant::now();
    let timeout = Duration::from_secs(5);

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
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        // If we would block, check for timeout and continue
                        if start_time.elapsed() >= timeout {
                            eprintln!("Please check talk daemon status. Cannot connect to it!");
                            process::exit(128);
                        }

                        std::thread::sleep(Duration::from_millis(100));
                        continue;
                    }
                    Err(e) => {
                        eprintln!("Error receiving message: {}", e);
                        return Err(TalkError::IoError(e));
                    }
                }
            }
            Err(e) => {
                eprintln!("Error sending message: {}", e);
                return Err(TalkError::IoError(e));
            }
        }
    }
    Ok(())
}

/// Registers signal handlers for specific signals (SIGINT, SIGQUIT, SIGPIPE).
/// The signal handler function is `handle_signals`.
///
/// # Errors
///
/// Logs an error message if registering a signal handler fails.
pub fn register_signals() {
    unsafe {
        // List of signals to register handlers for
        let signals = &[SIGINT, SIGQUIT, SIGPIPE];

        // Register the `handle_signals` function for each signal
        for &sig in signals {
            if signal(sig, handle_signals as usize) == libc::SIG_ERR {
                eprintln!("Failed to register signal handler for signal {}", sig);
            }
        }
    }
}

/// Handles incoming signals by setting the interrupt flag and exiting the process.
///
/// # Arguments
///
/// * `signal_code` - The signal code received (e.g., from `SIGINT`).
///
/// # Errors
///
/// Logs an error message if sending or receiving messages fails.
pub fn handle_signals(signal_code: libc::c_int) {
    // Clear the terminal screen
    clear_terminal();
    eprintln!("Connection closed, exiting...");

    // Lock the DELETE_INVITATIONS mutex and check for an existing invitation
    if let Some((msg_bytes, socket, talkd_addr)) = DELETE_INVITATIONS.lock().unwrap().as_ref() {
        // Handle the deletion of invitations
        handle_delete_invitations(socket, msg_bytes, talkd_addr);
    }

    // Exit the process with a code indicating the signal received
    std::process::exit(128 + signal_code);
}

/// Clears the terminal screen by sending escape sequences.
///
/// # Errors
///
/// This function does not return errors, but prints to standard error if the terminal cannot be cleared.
fn clear_terminal() {
    // clear screeen
    eprint!("\x1B[2J\x1B[H");
}

/// Handles sending a delete request and waiting for the response from the talk daemon.
///
/// # Arguments
///
/// * `socket` - A reference to the socket used for communication.
/// * `msg_bytes` - The bytes of the message to send.
/// * `talkd_addr` - The address of the talk daemon to which the message is sent.
///
/// # Errors
///
/// Logs an error message if sending or receiving messages fails.
fn handle_delete_invitations(socket: &UdpSocket, msg_bytes: &[u8], talkd_addr: &SocketAddr) {
    loop {
        if let Err(e) = socket.send_to(msg_bytes, talkd_addr) {
            eprintln!("Error sending message: {}", e);
            break;
        }

        let mut buf = [0; 1024];
        match socket.recv_from(&mut buf) {
            Ok((amt, _)) => {
                if let Ok(ctl_res) =
                    CtlRes::from_bytes(&buf[..amt]).map_err(|e| TalkError::Other(e.to_string()))
                {
                    if ctl_res.r#type == MessageType::Delete {
                        break;
                    }
                }
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => {
                eprintln!("Error receiving message: {}", e);
                break;
            }
        }
    }
}

/// Clears the terminal screen and exits the program.
///
/// This function prints a message indicating that the connection is closed,
/// clears the terminal, and terminates the process with exit code `3`.
fn handle_connection_close() {
    clear_terminal();
    io::stdout().flush().ok();
    eprintln!("Connection closed, exiting...");
    process::exit(130);
}

/// Retrieves the terminal size (width and height in characters) via an `ioctl` system call.
///
/// # Returns
///
/// A tuple `(width, height)`. Defaults to `(80, 24)` if the size cannot be retrieved.
///
/// # Errors
///
/// Prints an error message if the `ioctl` call fails.
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
