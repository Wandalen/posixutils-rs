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
use libc::{addrinfo, getaddrinfo, sockaddr_in, AF_INET, STATX_CTIME};
use libc::{c_char, servent};
use plib::PROJECT_NAME;
use std::net::{IpAddr, Ipv4Addr, TcpListener};
use std::ptr;
use std::{
    ffi::{CStr, CString},
    net::{SocketAddr, UdpSocket},
    time::Duration,
};

use byteorder::BigEndian;
use byteorder::ByteOrder;
use libc::{c_uchar, sa_family_t};
use std::io::{Cursor, Error};
use std::io::{ErrorKind, Write};
use std::mem::size_of;

static ANSWERS: [&str; 9] = [
    "answer #0",                                          // SUCCESS
    "Your party is not logged on",                        // NOT_HERE
    "Target machine is too confused to talk to us",       // FAILED
    "Target machine does not recognize us",               // MACHINE_UNKNOWN
    "Your party is refusing messages",                    // PERMISSION_REFUSED
    "Target machine cannot handle remote talk",           // UNKNOWN_REQUEST
    "Target machine indicates protocol mismatch",         // BADVERSION
    "Target machine indicates protocol botch (addr)",     // BADADDR
    "Target machine indicates protocol botch (ctl_addr)", // BADCTLADDR
];

const N_ANSWERS: usize = 9;
const BUFFER_SIZE: usize = 12;
const TALK_VERSION: u8 = 1;

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

    let mut msg = CTL_MSG {
        vers: 1,
        r#type: 0,
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
        l_name: string_to_c_string(""),
        r_name: string_to_c_string(""),
        r_tty: [0; 12],
    };

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

    let current_state: &str;

    let (my_machine_name, his_machine_name) =
        get_names(&mut msg, args.address.as_ref().unwrap(), args.ttyname)?;

    let my_machine_addr = get_addrs(&mut msg, &my_machine_name, &his_machine_name)?;

    let (socket_addr, listener) = open_sockt(my_machine_addr)?;
    let (ctl_addr, socket) = open_ctl(my_machine_addr)?;

    let ctl_addr_data = msg.create_ctl_addr(ctl_addr);
    let addr_data =
        msg.create_sockaddr_data(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 2));

    msg.addr.sa_data = addr_data;
    msg.ctl_addr.sa_data = ctl_addr_data;

    look_for_invite(&mut msg, &socket, &mut res);

    // current_state = "Trying to connect to your party's talk daemon";

    // if res.r#type == 1 {
    //     announce(&mut msg, &socket, &mut res);

    //     let remote_id = res.id_num;

    //     if res.answer != 0 {
    //         // println!("{:?}", ANSWERS.get(res.answer));
    //     }
    //     leave_invite(&mut msg, &socket, &mut res);
    //     let local_id = res.id_num;
    // }

    println!("[Waiting for your party to respond]");

    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                // Successfully connected
                dbg!("connected");
                break;
            }
            Err(e) if e.kind() == ErrorKind::Interrupted => {
                // Retry on EINTR (signal interruption)
                continue;
            }
            Err(e) => {
                // p_error(&format!("Unable to connect with your party: {}", e));
                // return Err(e);
            }
        }
    }

    // send_delete(&mut msg, &socket, &mut res);

    Ok(())
}

// Determine the local and remote user, tty, and machines
fn get_names(
    msg: &mut CTL_MSG,
    address: &str,
    ttyname: Option<String>,
) -> Result<(String, String), std::io::Error> {
    // Get the current user's name
    let my_name = unsafe {
        let login_name = libc::getlogin();
        if !login_name.is_null() {
            CStr::from_ptr(login_name).to_string_lossy().into_owned()
        } else {
            let pw = libc::getpwuid(libc::getuid());
            if pw.is_null() {
                return Err(Error::new(
                    std::io::ErrorKind::NotFound,
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
        let result = unsafe { libc::gethostname(buffer.as_mut_ptr(), buffer.len()) };

        if result == 0 {
            let c_str = unsafe { CStr::from_ptr(buffer.as_ptr()) };
            c_str.to_string_lossy().into_owned()
        } else {
            return Err(Error::new(
                std::io::ErrorKind::Other,
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
    // msg.id_num = 0u32.to_be() as u64;
    msg.l_name = string_to_c_string(&my_name);
    msg.r_name = string_to_c_string(&his_name);
    msg.r_tty = string_to_c_string(&ttyname.unwrap_or_default());

    Ok((my_machine_name, his_machine_name))
}

fn get_addrs(
    msg: &mut CTL_MSG,
    my_machine_name: &str,
    his_machine_name: &str,
) -> Result<Ipv4Addr, std::io::Error> {
    let service = CString::new("ntalk").expect("CString::new failed");
    let protocol = CString::new("udp").expect("CString::new failed");

    //todo: add IDN
    let lhost = CString::new(my_machine_name).expect("CString::new failed");
    let rhost = CString::new(his_machine_name).expect("CString::new failed");

    let mut my_machine_addr: Ipv4Addr = Ipv4Addr::UNSPECIFIED;
    let mut his_machine_addr: Ipv4Addr = Ipv4Addr::UNSPECIFIED;

    msg.pid = unsafe { libc::getpid() };

    let hints = libc::addrinfo {
        ai_family: libc::AF_INET, // IPv4 only
        ai_socktype: libc::SOCK_DGRAM,
        ai_flags: libc::AI_CANONNAME,
        ai_protocol: 0,
        ai_addrlen: 0,
        ai_canonname: ptr::null_mut(),
        ai_addr: ptr::null_mut(),
        ai_next: ptr::null_mut(),
    };

    unsafe {
        // Get local host address
        let mut res: *mut addrinfo = ptr::null_mut();
        let err = libc::getaddrinfo(lhost.as_ptr(), ptr::null(), &hints, &mut res);
        if err != 0 {
            eprintln!(
                "talk: {}: {}",
                my_machine_name,
                std::ffi::CStr::from_ptr(libc::gai_strerror(err))
                    .to_str()
                    .unwrap()
            );
            std::process::exit(-1);
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
            std::process::exit(-1);
        }

        // Get remote host address
        if rhost != lhost {
            let mut res_remote: *mut addrinfo = ptr::null_mut();
            let err_remote = getaddrinfo(rhost.as_ptr(), ptr::null(), &hints, &mut res_remote);
            if err_remote != 0 {
                eprintln!(
                    "talk: {}: {}",
                    his_machine_name,
                    std::ffi::CStr::from_ptr(libc::gai_strerror(err_remote))
                        .to_str()
                        .unwrap()
                );
                std::process::exit(-1);
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
                std::process::exit(-1);
            }
        } else {
            his_machine_addr = my_machine_addr;
        }
    }

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

    Ok(my_machine_addr)
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

    std::process::exit(exit_code)
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

    pub fn create_sockaddr_data(&self, addr: SocketAddr) -> [u8; 14] {
        let mut sa_data: [u8; 14] = [0; 14];

        if let std::net::IpAddr::V4(ipv4) = addr.ip() {
            let ip_bytes = ipv4.octets();
            sa_data[..4].copy_from_slice(&ip_bytes);

            let port_bytes = addr.port().to_be_bytes();
            sa_data[4..6].copy_from_slice(&port_bytes);
        }

        sa_data
    }

    pub fn create_ctl_addr(&self, addr: SocketAddr) -> [u8; 14] {
        let mut ctl_addr: [u8; 14] = [0; 14];
        if let std::net::IpAddr::V4(ipv4) = addr.ip() {
            let ip_bytes = ipv4.octets();

            let port_bytes = addr.port().to_be_bytes();
            ctl_addr[0..2].copy_from_slice(&port_bytes);

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

fn open_sockt(my_machine_addr: Ipv4Addr) -> Result<(SocketAddr, TcpListener), std::io::Error> {
    let listener = TcpListener::bind((my_machine_addr, 0))?;
    let addr = listener.local_addr()?;

    println!("TCP Socket bound to address: {}", addr);

    Ok((addr, listener))
}

fn open_ctl(my_machine_addr: Ipv4Addr) -> Result<(SocketAddr, UdpSocket), std::io::Error> {
    let socket = UdpSocket::bind((my_machine_addr, 0))?;
    socket.set_nonblocking(true)?;

    let addr = socket.local_addr()?;

    println!("UDP Socket bound to address: {}", addr);

    Ok((addr, socket))
}

fn look_for_invite(msg: &mut CTL_MSG, socket: &UdpSocket, res: &mut CTL_RES) {
    // LOOK_UP
    perform_socket_operation(msg, 1, socket, res).unwrap();

    if res.answer == 0 {
        dbg!(100);
    }
}

fn leave_invite(msg: &mut CTL_MSG, socket: &UdpSocket, res: &mut CTL_RES) {
    // LEAVE_INVITE
    perform_socket_operation(msg, 0, socket, res).unwrap();
}

fn announce(msg: &mut CTL_MSG, socket: &UdpSocket, res: &mut CTL_RES) {
    // ANNOUNCE
    perform_socket_operation(msg, 3, socket, res).unwrap();
}

fn send_delete(msg: &mut CTL_MSG, socket: &UdpSocket, res: &mut CTL_RES) {
    // msg.id_num = remote_id;
    // remote
    // msg.addr.sa_data = his_machine_addr;
    // daemon_addr.sin_addr = my_machine_addr;
    // DELETE
    perform_socket_operation(msg, 2, socket, res).unwrap();
}
fn perform_socket_operation(
    msg: &mut CTL_MSG,
    msg_type: u8,
    socket: &UdpSocket,
    res: &mut CTL_RES,
) -> std::io::Result<()> {
    //todo: talkd_addr changable
    let talkd_addr: SocketAddr = "0.0.0.0:518".parse().unwrap();
    // socket.set_nonblocking(true)?;

    let msg_bytes = msg.to_bytes();

    dbg!(&msg_bytes);
    // println!("[Service connection established.]");
    loop {
        match socket.send_to(&msg_bytes, talkd_addr) {
            Ok(_) => {
                let mut buf = [0; 1024];
                match socket.recv_from(&mut buf) {
                    Ok((amt, src)) => {
                        println!("Received {} bytes from {}: {:?}", amt, src, &buf[..amt]);
                        let ctl_res = bytes_to_ctl_res(&buf[..amt]);
                        res.answer = ctl_res.answer;
                        res.r#type = ctl_res.r#type;
                        // dbg!(res.r#type);
                        if msg_type == 1 {
                            break;
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        // println!("Resource temporarily unavailable while receiving, retrying...");
                        // std::thread::sleep(Duration::from_secs(1));
                        continue;
                    }
                    Err(e) => {
                        eprintln!("Error receiving message: {}", e);
                        break;
                    }
                }
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
    // Receive the response

    Ok(())
}
