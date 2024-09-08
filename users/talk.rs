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

use clap::Parser;
use gettextrs::{bind_textdomain_codeset, setlocale, textdomain, LocaleCategory};
use libc::{addrinfo, getaddrinfo, AF_INET, AI_PASSIVE, SOCK_STREAM};
use libc::{c_char, servent, sockaddr_in};
use plib::PROJECT_NAME;
use std::ptr;
use std::{
    ffi::{CStr, CString},
    net::{Ipv4Addr, SocketAddr, TcpListener, UdpSocket},
    time::Duration,
};

const BUFFER_SIZE: usize = 12;
use byteorder::ByteOrder;
use byteorder::{BigEndian, WriteBytesExt};
use libc::{c_uchar, sa_family_t};
use std::io::Cursor;
use std::io::Write;
use std::mem::size_of;

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
    if args.address.is_none() {
        eprintln!("Usage: talk user [ttyname]");
        std::process::exit(-1);
    }

    let is_tty = atty::is(atty::Stream::Stdin);
    if !is_tty {
        println!("not a tty");
        std::process::exit(1);
    }
    match get_names(args.address.as_ref().unwrap(), args.ttyname) {
        Ok((his_name, his_machine_name)) => {
            println!("User: {}", his_name);
            println!("Machine: {}", his_machine_name);
        }
        Err(e) => println!("Error: {}", e),
    }

    Ok(())
}

// Determine the local and remote user, tty, and machines
fn get_names(address: &str, ttyname: Option<String>) -> Result<(String, String), String> {
    // Get the current user's name
    let my_name = unsafe {
        let login_name = libc::getlogin();
        if !login_name.is_null() {
            CStr::from_ptr(login_name).to_string_lossy().into_owned()
        } else {
            let pw = libc::getpwuid(libc::getuid());
            if pw.is_null() {
                return Err("You don't exist. Go away.".to_string());
            } else {
                CStr::from_ptr((*pw).pw_name).to_string_lossy().into_owned()
            }
        }
    };

    // Get the local machine name
    // todo: allocate enought sized buffer - safety
    let my_machine_name = {
        let mut buffer = vec![0 as c_char; 256];
        let result = unsafe { libc::gethostname(buffer.as_mut_ptr(), buffer.len()) };

        if result == 0 {
            let c_str = unsafe { CStr::from_ptr(buffer.as_ptr()) };
            c_str.to_string_lossy().into_owned()
        } else {
            return Err("Cannot get local hostname".to_string());
        }
    };

    let have_at_symbol = address.find(|c| "@:!.".contains(c));

    let (his_name, his_machine_name) = if let Some(index) = have_at_symbol {
        let delimiter = address.chars().nth(index).unwrap();
        if delimiter == '@' {
            let (user, host) = address.split_at(index);
            (user.to_string(), host[1..].to_string())
        } else {
            let (host, user) = address.split_at(index);
            (user[1..].to_string(), host.to_string())
        }
    } else {
        // local for local talk
        (address.to_string(), my_machine_name.clone())
    };

    let his_tty = ttyname.unwrap_or_default();

    get_addrs(&my_machine_name, &his_machine_name);

    Ok((his_name, his_machine_name))
}

fn get_addrs(my_machine_name: &str, his_machine_name: &str) {
    let service = CString::new("ntalk").expect("CString::new failed");
    let protocol = CString::new("udp").expect("CString::new failed");

    //todo: add IDN
    let lhost = my_machine_name;
    let rhost = his_machine_name;

    let mut res: *mut addrinfo = ptr::null_mut();
    let lhost_cstr = CString::new(lhost).expect("CString::new failed");

    // Call getservbyname
    let result: *mut servent = unsafe { libc::getservbyname(service.as_ptr(), protocol.as_ptr()) };

    if result.is_null() {
        eprintln!("talk: {}/{}: service is not registered.", "ntalk", "udp");
        std::process::exit(1);
    }

    let daemon_port = unsafe {
        let servent = *result;
        let port = servent.s_port;
        u16::from_be(port as u16)
    };
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // parse command line arguments
    let args = Args::parse();

    setlocale(LocaleCategory::LcAll, "");
    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;

    let mut exit_code = 0;

    if let Err(err) = talk(args) {
        exit_code = 1;
        eprint!("{}", err);
    }

    let mut res = CTL_RES {
        vers: 0,
        r#type: 0,
        answer: 0,
        pad: 0,
        id_num: 0,
        addr: Osockaddr {
            sa_family: 0,
            sa_data: [0; 14],
        },
    };

    look_for_invite(&mut res);
    announce_invite(&mut res);

    std::process::exit(exit_code)
}

pub enum MessageType {
    LeaveInvite,
    LookUp,
    Delete,
    Announce,
}

impl MessageType {
    pub fn as_u8(&self) -> u8 {
        match self {
            MessageType::LeaveInvite => 0,
            MessageType::LookUp => 1,
            MessageType::Delete => 2,
            MessageType::Announce => 3,
        }
    }
}

#[repr(C, packed(1))]
pub struct Osockaddr {
    pub sa_family: sa_family_t,
    pub sa_data: [u8; 14],
}

#[repr(C, packed(1))]
struct CTL_MSG {
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
pub struct CTL_RES {
    pub vers: c_uchar,
    pub r#type: c_uchar,
    pub answer: c_uchar,
    pub pad: c_uchar,
    pub id_num: u64,
    pub addr: Osockaddr,
}

impl CTL_MSG {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = vec![0u8; size_of::<CTL_MSG>()];
        let mut cursor = Cursor::new(&mut bytes[..]);

        cursor.write_all(&self.vers.to_be_bytes()).unwrap();
        cursor.write_all(&self.r#type.to_be_bytes()).unwrap();
        cursor.write_all(&self.answer.to_be_bytes()).unwrap();
        cursor.write_all(&self.pad.to_be_bytes()).unwrap();
        cursor.write_all(&self.id_num.to_be_bytes()).unwrap();
        cursor.write_all(&self.addr.sa_data).unwrap();
        cursor.write_all(&self.ctl_addr.sa_data).unwrap();
        cursor.write_all(&self.pid.to_be_bytes()).unwrap();
        cursor
            .write_all(&self.l_name.iter().map(|&b| b as u8).collect::<Vec<u8>>())
            .unwrap();
        cursor
            .write_all(&self.r_name.iter().map(|&b| b as u8).collect::<Vec<u8>>())
            .unwrap();
        cursor
            .write_all(&self.r_tty.iter().map(|&b| b as u8).collect::<Vec<u8>>())
            .unwrap();

        bytes
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

    pub fn create_ctl_addr(&self, ip: &str, port: u16) -> [u8; 14] {
        let mut ctl_addr: [u8; 14] = [0; 14];

        ctl_addr[0..2].copy_from_slice(&port.to_be_bytes());

        let ip_bytes: Vec<u8> = ip
            .split('.')
            .map(|s| s.parse::<u8>().unwrap_or(0))
            .collect();

        if ip_bytes.len() == 4 {
            ctl_addr[2..6].copy_from_slice(&ip_bytes);
        }

        ctl_addr
    }
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

fn send_message(
    socket: &UdpSocket,
    addr: &SocketAddr,
    msg_type: MessageType,
) -> std::io::Result<()> {
    let mut msg = CTL_MSG {
        vers: 1,
        r#type: msg_type.as_u8(),
        answer: 0,
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
        l_name: string_to_c_string("egor"),
        r_name: string_to_c_string("egor"),
        r_tty: [0; 12],
    };

    let ctl_addr_data = msg.create_ctl_addr(&"127.0.1.1", socket.local_addr().unwrap().port());
    let addr_data = msg.create_sockaddr_data(&"0.0.0.0", 2);
    msg.pid = unsafe { libc::getpid() };

    msg.addr.sa_data = addr_data;
    msg.ctl_addr.sa_data = ctl_addr_data;

    let msg_bytes = msg.to_bytes();

    loop {
        match socket.send_to(&msg_bytes, addr) {
            Ok(_) => {
                println!("[Service connection established.]");
                break;
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // println!("Resource temporarily unavailable, retrying...");
                std::thread::sleep(Duration::from_secs(1));
            }
            Err(e) => {
                eprintln!("Error sending message: {}", e);
                return Err(e);
            }
        }
    }

    Ok(())
}

fn receive_message(socket: &UdpSocket, response: &mut CTL_RES) -> bool {
    // Receive the response
    loop {
        let mut buf = [0; 1024];
        match socket.recv_from(&mut buf) {
            Ok((amt, src)) => {
                println!("Received {} bytes from {}: {:?}", amt, src, &buf[..amt]);
                let ctl_res = bytes_to_ctl_res(&buf[..amt]);
                dbg!(ctl_res.vers);
                dbg!(ctl_res.r#type);
                dbg!(ctl_res.answer);
                response.answer = ctl_res.answer;
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // println!("Resource temporarily unavailable while receiving, retrying...");
                std::thread::sleep(Duration::from_secs(1));
            }
            Err(e) => {
                eprintln!("Error receiving message: {}", e);
                break false;
            }
        }
    }
}
fn bytes_to_ctl_res(bytes: &[u8]) -> CTL_RES {
    // Extract the ID number (big-endian)
    let id_num = BigEndian::read_u64(&bytes[4..12]);

    // Extract the sockaddr (family + data)
    let sa_family = bytes[12] as sa_family_t;
    // let mut sa_data = [0u8; 14];
    // sa_data.copy_from_slice(&bytes[13..24]);

    // Create the Osockaddr
    let addr = Osockaddr {
        sa_family,
        sa_data: [0; 14],
    };

    // Return the struct populated with values from the byte slice
    CTL_RES {
        vers: bytes[0],
        r#type: bytes[1],
        answer: bytes[2],
        pad: bytes[3],
        id_num,
        addr,
    }
}
fn perform_socket_operation(addr_str: &str, message_type: MessageType, response: &mut CTL_RES)  -> std::io::Result<()>{
    let talkd_addr: SocketAddr = addr_str.parse().unwrap();
    let local_socket = UdpSocket::bind("127.0.1.1:0")?;
    local_socket.set_nonblocking(true)?;

    send_message(&local_socket, &talkd_addr, message_type)?;
    receive_message(&local_socket, response);

    Ok(())
}

fn look_for_invite(response: &mut CTL_RES) {
    perform_socket_operation("0.0.0.0:518", MessageType::LookUp, response).unwrap();
}

fn announce_invite(response: &mut CTL_RES) {
    perform_socket_operation("0.0.0.0:518", MessageType::LeaveInvite, response).unwrap();
}
