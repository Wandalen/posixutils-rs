extern crate clap;
extern crate libc;
extern crate plib;

use clap::Parser;
use core::fmt;
use gettextrs::{bind_textdomain_codeset, setlocale, textdomain, LocaleCategory};
use libc::{fstat, PF_UNSPEC, SOCK_DGRAM, SOCK_STREAM};
use plib::PROJECT_NAME;
use std::{
    collections::BTreeMap,
    env,
    ffi::{CStr, CString},
    fs::{self, File},
    io::{self, BufRead, Write},
    net::{IpAddr, UdpSocket},
    os::unix::io::AsRawFd,
    path::{Component, Path, PathBuf},
    sync::mpsc,
    thread,
    time::Duration,
};

const PROC_PATH: &'static str = "/proc";
const PROC_MOUNTS: &'static str = "/proc/mounts";

#[derive(Clone, Debug, Default, PartialEq)]
enum ProcType {
    #[default]
    Normal = 0,
    Mount = 1,
    Knfsd = 2,
    Swap = 3,
}

#[derive(Clone, Debug, Default, PartialEq)]
enum NameSpace {
    #[default]
    File = 0,
    Tcp = 1,
    Udp = 2,
}

#[derive(Clone, Debug, Default, PartialEq)]
enum Access {
    Cwd = 1,
    Exe = 2,
    #[default]
    File = 4,
    Root = 8,
    Mmap = 16,
    Filewr = 32,
}

#[derive(Clone, Debug)]
struct IpConnections {
    names: Names,
    lcl_port: u64,
    rmt_port: u64,
    rmt_addr: IpAddr,
    next: Option<Box<IpConnections>>,
}

impl Default for IpConnections {
    fn default() -> Self {
        IpConnections {
            names: Names::default(),
            lcl_port: 0,
            rmt_port: 0,
            rmt_addr: IpAddr::V4("0.0.0.0".parse().unwrap()),
            next: None,
        }
    }
}

impl IpConnections {
    fn new(names: Names, lcl_port: u64, rmt_port: u64, rmt_addr: IpAddr) -> Self {
        IpConnections {
            names,
            lcl_port,
            rmt_port,
            rmt_addr,
            next: None,
        }
    }
    fn add_ip_conn(&mut self, names: Names, lcl_port: u64, rmt_port: u64, rmt_addr: IpAddr) {
        let new_node = Box::new(IpConnections {
            names,
            lcl_port,
            rmt_port,
            rmt_addr,
            next: self.next.take(),
        });

        self.next = Some(new_node);
    }

    fn iter(&self) -> IpConnectionsIterator {
        IpConnectionsIterator {
            current: Some(self),
        }
    }
}

struct IpConnectionsIterator<'a> {
    current: Option<&'a IpConnections>,
}

impl<'a> Iterator for IpConnectionsIterator<'a> {
    type Item = &'a IpConnections;

    fn next(&mut self) -> Option<Self::Item> {
        self.current.map(|node| {
            self.current = node.next.as_deref();
            node
        })
    }
}

#[derive(Clone, Debug, Default)]
struct Procs {
    pid: i32,
    uid: u32,
    access: Access,
    proc_type: ProcType,
    username: Option<i8>,
    command: String,
    next: Option<Box<Procs>>,
}

impl Procs {
    fn new(pid: i32, uid: u32, access: Access, proc_type: ProcType, command: String) -> Self {
        Self {
            pid,
            uid,
            access,
            proc_type,
            username: None,
            command,
            next: None,
        }
    }
}

#[derive(Debug, Default, Clone)]
struct UnixSocketList {
    name: String,
    device_id: u64,
    inode: u64,
    net_inode: u64,
    next: Option<Box<UnixSocketList>>,
}

impl UnixSocketList {
    fn new(name: String, device_id: u64, inode: u64, net_inode: u64) -> Self {
        UnixSocketList {
            name,
            device_id,
            inode,
            net_inode,
            next: None,
        }
    }

    fn add_socket(&mut self, name: String, device_id: u64, inode: u64, net_inode: u64) {
        let new_node = Box::new(UnixSocketList {
            name,
            device_id,
            net_inode,
            inode,
            next: self.next.take(),
        });

        self.next = Some(new_node);
    }

    fn iter(&self) -> UnixSocketListIterator {
        UnixSocketListIterator {
            current: Some(self),
        }
    }
}

struct UnixSocketListIterator<'a> {
    current: Option<&'a UnixSocketList>,
}

impl<'a> Iterator for UnixSocketListIterator<'a> {
    type Item = &'a UnixSocketList;

    fn next(&mut self) -> Option<Self::Item> {
        self.current.map(|node| {
            self.current = node.next.as_deref();
            node
        })
    }
}

#[derive(Debug, Default)]
struct InodeList {
    name: Names,
    device_id: u64,
    inode: u64,
    next: Option<Box<InodeList>>,
}

impl InodeList {
    fn new(name: Names, device_id: u64, inode: u64) -> Self {
        InodeList {
            name,
            device_id,
            inode,
            next: None,
        }
    }

    fn add_inode(&mut self, name: Names, device_id: u64, inode: u64) {
        let new_node = Box::new(InodeList {
            name,
            device_id,
            inode,
            next: self.next.take(),
        });

        self.next = Some(new_node);
    }

    fn iter(&self) -> InodeListIterator {
        InodeListIterator {
            current: Some(self),
        }
    }
}

struct InodeListIterator<'a> {
    current: Option<&'a InodeList>,
}

impl<'a> Iterator for InodeListIterator<'a> {
    type Item = &'a InodeList;

    fn next(&mut self) -> Option<Self::Item> {
        self.current.map(|node| {
            self.current = node.next.as_deref();
            node
        })
    }
}

#[derive(Debug, Default, Clone)]
struct MountList {
    mountpoints: Vec<PathBuf>,
    next: Option<Box<MountList>>,
}

struct LibcStat {
    inner: libc::stat,
}

impl fmt::Debug for LibcStat {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}

impl Default for LibcStat {
    fn default() -> Self {
        LibcStat {
            inner: unsafe { std::mem::zeroed() },
        }
    }
}

impl Clone for LibcStat {
    fn clone(&self) -> Self {
        LibcStat { inner: self.inner }
    }
}

#[derive(Debug, Clone, Default)]
struct Names {
    filename: PathBuf,
    name_space: NameSpace,
    matched_procs: Vec<Procs>,
    st: LibcStat,
    next: Option<Box<Names>>,
}

impl Names {
    fn new(
        filename: PathBuf,
        name_space: NameSpace,
        st: libc::stat,
        matched_procs: Vec<Procs>,
    ) -> Self {
        Names {
            filename,
            name_space,
            st: LibcStat { inner: st },
            matched_procs,
            next: None,
        }
    }

    fn add_procs(&mut self, proc: Procs) {
        self.matched_procs.push(proc);
    }
    fn iter(&self) -> NamesIterator {
        NamesIterator {
            current: Some(self),
        }
    }
}

struct NamesIterator<'a> {
    current: Option<&'a Names>,
}

impl<'a> Iterator for NamesIterator<'a> {
    type Item = &'a Names;

    fn next(&mut self) -> Option<Self::Item> {
        self.current.map(|node| {
            self.current = node.next.as_deref();
            node
        })
    }
}

#[derive(Debug, Default)]
struct DeviceList {
    name: Names,
    device_id: u64,
    next: Option<Box<DeviceList>>,
}

impl DeviceList {
    fn new(name: Names, device_id: u64) -> Self {
        DeviceList {
            name,
            device_id,
            next: None,
        }
    }
    fn iter(&self) -> DeviceListIterator {
        DeviceListIterator {
            current: Some(self),
        }
    }
}

struct DeviceListIterator<'a> {
    current: Option<&'a DeviceList>,
}

impl<'a> Iterator for DeviceListIterator<'a> {
    type Item = &'a DeviceList;

    fn next(&mut self) -> Option<Self::Item> {
        self.current.map(|node| {
            self.current = node.next.as_deref();
            node
        })
    }
}

/// fuser - list process IDs of all processes that have one or more files open
#[derive(Parser, Debug)]
#[command(author, version, about, long_about)]
struct Args {
    /// The file is treated as a mount point and the utility shall report on any files open in the file system.
    #[arg(short = 'c')]
    mount: bool,
    /// The report shall be only for the named files.
    #[arg(short = 'f')]
    named_files: bool,
    /// The user name, in parentheses, associated with each process ID written to standard output shall be written to standard error.
    #[arg(short = 'u')]
    user: bool,

    #[arg(required = true, name = "FILE", num_args(0..))]
    /// A pathname on which the file or file system is to be reported.
    file: Vec<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Args {
        mount, user, file, ..
    } = Args::parse();
    // init default structures
    let mut names = Names::default();
    let mut unix_socket_list = UnixSocketList::default();
    let mut mount_list = MountList::default();
    let mut device_list = DeviceList::default();
    let mut inode_list = InodeList::default();
    let mut tcp_connection_list = IpConnections::default();
    let mut udp_connection_list = IpConnections::default();

    names.name_space = NameSpace::default();
    names.filename = PathBuf::from(&file[0]);

    fill_unix_cache(&mut unix_socket_list)?;
    let expanded_path = expand_path(&file[0])?;

    let net_dev = find_net_dev()?;

    if let Some(name_str) = names.filename.to_str() {
        if name_str.contains("tcp") {
            names.name_space = NameSpace::Tcp;
        } else if name_str.contains("udp") {
            names.name_space = NameSpace::Udp;
        } else if name_str.contains("file") {
            names.name_space = NameSpace::File;
        }
    }

    match names.name_space {
        NameSpace::File => {
            let st = timeout(&names.filename.to_string_lossy(), 5)?;

            if mount {
                read_proc_mounts(&mut mount_list)?;
                inode_list = InodeList::new(names.clone(), st.st_dev, 0);
                // adding device to DeviceList
                let device_id;
                if (libc::S_IFBLK == names.st.inner.st_mode) {
                    device_id = st.st_rdev;
                } else {
                    device_id = st.st_dev;
                }

                device_list = DeviceList::new(names.clone(), device_id);
            } else {
                inode_list = InodeList::new(names.clone(), st.st_dev, st.st_ino);
                //adding inode to InodeList
                for unix_socket in unix_socket_list.iter() {
                    if unix_socket.device_id == names.st.inner.st_dev
                        && unix_socket.inode == names.st.inner.st_ino
                    {
                        InodeList::add_inode(
                            &mut inode_list,
                            names.clone(),
                            net_dev,
                            unix_socket.net_inode,
                        );
                    }
                }
            }
        }
        NameSpace::Tcp => {
            let tcp_connection_list = parse_inet(&mut names, &mut tcp_connection_list)?;
            inode_list = find_net_sockets(&mut inode_list, &tcp_connection_list, "tcp", net_dev)?;
        }
        NameSpace::Udp => {
            let udp_connection_list = parse_inet(&mut names, &mut udp_connection_list)?;
            inode_list = find_net_sockets(&mut inode_list, &udp_connection_list, "udp", net_dev)?;
        }
    }

    scan_procs(
        &mut names,
        &inode_list,
        &device_list,
        &unix_socket_list,
        net_dev,
    )?;
    // scan_mounts(&mut names, &inode_list, &device_list)?;

    print_matches(names, user, expanded_path)?;

    setlocale(LocaleCategory::LcAll, "");
    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;

    let exit_code = 0;

    std::process::exit(exit_code)
}

fn print_matches(names: Names, user: bool, expanded_path: PathBuf) -> Result<(), io::Error> {
    let mut proc_map: BTreeMap<i32, (String, u32)> = BTreeMap::new();
    let mut name_has_procs = false;
    for name in names.iter() {
        for procs in name.matched_procs.iter() {
            if procs.proc_type == ProcType::Normal {
                name_has_procs = true;
            }
            let entry = proc_map
                .entry(procs.pid)
                .or_insert((String::new(), procs.uid));

            match procs.access {
                Access::Root => entry.0.push_str("r"),
                Access::Cwd => entry.0.push_str("c"),
                Access::Exe => entry.0.push_str("e"),
                Access::Mmap => entry.0.push_str("m"),
                Access::File => (),
                Access::Filewr => (),
            }
        }
    }

    if !name_has_procs {
        // exit if no processes matched
        return Ok(());
    }
    eprint!("{}: ", expanded_path.display());
    io::stderr().flush()?;

    let mut output = String::new();
    let mut max_pid_length = 0;

    // First pass to determine the maximum PID length for alignment
    for (pid, (_, _)) in proc_map.clone() {
        let pid_len = pid.to_string().len();
        if pid_len > max_pid_length {
            max_pid_length = pid_len;
        }
    }

    for (pid, (access, _)) in proc_map {
        let pid_str = format!("{:width$}", pid, width = max_pid_length);
        let access_str = access.to_string();

        let formatted_str = format!("{}{}", pid_str, access_str);
        output.push_str(&formatted_str);
        output.push_str(" "); // Add space between entries

        // Flush stderr after each write to ensure all output is visible
        io::stderr().flush()?;
    }

    // Print to stdout and flush
    print!("{}", output.trim_end());
    io::stdout().flush()?;

    // Print newline to stderr and flush
    eprintln!("\n");
    io::stderr().flush()?;
    Ok(())
}

fn scan_procs(
    names: &mut Names,
    inode_list: &InodeList,
    device_list: &DeviceList,
    unix_socket_list: &UnixSocketList,
    net_dev: u64,
) -> Result<(), io::Error> {
    let my_pid = std::process::id() as i32;
    for entry in fs::read_dir(PROC_PATH)? {
        let entry = entry?;
        let filename = entry
            .file_name()
            .into_string()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid file name"))?;
        if let Ok(pid) = filename.parse::<i32>() {
            if pid == my_pid {
                continue;
            }

            let st = timeout(&entry.path().to_string_lossy(), 5)?;
            let uid = st.st_uid;

            let cwd_stat = match get_pid_stat(pid, "/cwd") {
                Ok(stat) => stat,
                Err(_) => continue,
            };

            let exe_stat = match get_pid_stat(pid, "/exe") {
                Ok(stat) => stat,
                Err(_) => continue,
            };
            let root_stat = match get_pid_stat(pid, "/root") {
                Ok(stat) => stat,
                Err(_) => continue,
            };
            check_root_access(names, pid, uid, &root_stat, device_list, inode_list)?;
            check_cwd_access(names, pid, uid, &cwd_stat, device_list, inode_list)?;
            check_exe_access(names, pid, uid, &exe_stat, device_list, inode_list)?;

            // check_dir(
            //     names,
            //     pid,
            //     "lib",
            //     device_list,
            //     inode_list,
            //     uid,
            //     Access::Mmap,
            //     unix_socket_list,
            //     net_dev,
            // )?;

            // check_dir(
            //     names,
            //     pid,
            //     "mmap",
            //     device_list,
            //     inode_list,
            //     uid,
            //     Access::Mmap,
            //     unix_socket_list,
            //     net_dev,
            // )?;

            // check_dir(
            //     names,
            //     pid,
            //     "fd",
            //     device_list,
            //     inode_list,
            //     uid,
            //     Access::File,
            //     unix_socket_list,
            //     net_dev,
            // )?;

            // check_map(names,pid, "maps", device_list, inode_list, uid, Access::Mmap)?;
        }
    }

    Ok(())
}

fn check_root_access(
    names: &mut Names,
    pid: i32,
    uid: u32,
    root_stat: &libc::stat,
    device_list: &DeviceList,
    inode_list: &InodeList,
) -> Result<(), std::io::Error> {
    for device in device_list.iter() {
        if root_stat.st_dev == device.device_id {
            add_process(names, pid, uid, Access::Root, ProcType::Normal, None);
            return Ok(());
        }
    }

    for inode in inode_list.iter() {
        if root_stat.st_dev == inode.device_id && root_stat.st_ino == inode.inode {
            add_process(names, pid, uid, Access::Root, ProcType::Normal, None);
            return Ok(());
        }
    }

    Ok(())
}

fn check_cwd_access(
    names: &mut Names,
    pid: i32,
    uid: u32,
    cwd_stat: &libc::stat,
    device_list: &DeviceList,
    inode_list: &InodeList,
) -> Result<(), std::io::Error> {
    for device in device_list.iter() {
        if cwd_stat.st_dev == device.device_id {
            add_process(names, pid, uid, Access::Cwd, ProcType::Normal, None);
            return Ok(());
        }
    }

    for inode in inode_list.iter() {
        if cwd_stat.st_dev == inode.device_id && cwd_stat.st_ino == inode.inode {
            add_process(names, pid, uid, Access::Cwd, ProcType::Normal, None);
            return Ok(());
        }
    }

    Ok(())
}

fn check_exe_access(
    names: &mut Names,
    pid: i32,
    uid: u32,
    exe_stat: &libc::stat,
    device_list: &DeviceList,
    inode_list: &InodeList,
) -> Result<(), std::io::Error> {
    for device in device_list.iter() {
        if exe_stat.st_dev == device.device_id {
            add_process(names, pid, uid, Access::Exe, ProcType::Normal, None);
            return Ok(());
        }
    }

    for inode in inode_list.iter() {
        if exe_stat.st_dev == inode.device_id && exe_stat.st_ino == inode.inode {
            add_process(names, pid, uid, Access::Exe, ProcType::Normal, None);
            return Ok(());
        }
    }

    Ok(())
}

// /proc/mount logic section
fn scan_mounts(
    names: &mut Names,
    inode_list: &InodeList,
    device_list: &DeviceList,
) -> Result<(), std::io::Error> {
    let file = File::open(PROC_MOUNTS).expect(&format!("Cannot open {}", PROC_MOUNTS));
    let reader = io::BufReader::new(file);

    for line in reader.lines() {
        let line = line?;
        let mut parts = line.split_whitespace();

        // Skip if there are not enough parts
        let find_mountp = match parts.nth(1) {
            Some(mount) => mount,
            None => continue,
        };

        let st = match timeout(find_mountp, 5) {
            Ok(stat) => stat,
            Err(_) => continue,
        };

        for device in device_list.iter() {
            if st.st_dev == device.device_id {
                add_process(
                    names,
                    0,
                    0,
                    Access::Mmap,
                    ProcType::Mount,
                    Some(find_mountp.to_string()),
                );
            }
        }
        for inode in inode_list.iter() {
            if st.st_dev == inode.device_id && st.st_ino == inode.inode {
                add_process(
                    names,
                    0,
                    0,
                    Access::Mmap,
                    ProcType::Mount,
                    Some(find_mountp.to_string()),
                );
            }
        }
    }

    Ok(())
}

fn add_process(
    names: &mut Names,
    pid: i32,
    uid: u32,
    access: Access,
    proc_type: ProcType,
    command: Option<String>,
) {
    let proc = Procs::new(pid, uid, access, proc_type, command.unwrap_or_default());
    names.add_procs(proc);
}

fn check_dir(
    names: &mut Names,
    pid: i32,
    dirname: &str,
    device_list: &DeviceList,
    inode_list: &InodeList,
    uid: u32,
    access: Access,
    unix_socket_list: &UnixSocketList,
    net_dev: u64,
) -> Result<(), std::io::Error> {
    let dir_path = format!("/proc/{}/{}", pid, dirname);
    if let Ok(dir_entries) = fs::read_dir(&dir_path) {
        for entry in dir_entries {
            let entry = entry?;
            let mut stat = match timeout(&entry.path().to_string_lossy(), 5) {
                Ok(stat) => stat,
                Err(_) => continue,
            };

            if stat.st_dev == net_dev {
                for unix_socket in unix_socket_list.iter() {
                    if (unix_socket.net_inode == stat.st_ino) {
                        stat.st_ino = unix_socket.inode;
                        stat.st_dev = unix_socket.device_id;
                        break;
                    }
                }
            }
            for device in device_list.iter() {
                match access {
                    Access::File => {
                        add_process(names, pid, uid, access.clone(), ProcType::Normal, None);
                    }
                    _ => add_process(names, pid, uid, access.clone(), ProcType::Normal, None),
                }
            }

            for inode in inode_list.iter() {
                if stat.st_ino == inode.inode {
                    match access {
                        Access::File => {
                            add_process(names, pid, uid, access.clone(), ProcType::Normal, None);
                        }
                        _ => add_process(names, pid, uid, access.clone(), ProcType::Normal, None),
                    }
                }
            }
        }
    } else {
        // eprintln!("Cannot open directory: {}", dir_path);
    }

    Ok(())
}

fn check_map(
    names: &mut Names,
    pid: i32,
    filename: &str,
    device_list: &DeviceList,
    inode_list: &InodeList,
    uid: u32,
    access: Access,
) -> Result<(), std::io::Error> {
    let pathname = format!("/proc/{}/{}", pid, filename);
    let file = File::open(&pathname)?;
    let reader = io::BufReader::new(file);

    for line in reader.lines() {
        let line = line?;
        let parts: Vec<&str> = line.split_whitespace().collect();
        let dev_info: Vec<&str> = parts[3].split(':').collect();
        if dev_info.len() == 2 {
            let tmp_maj = u32::from_str_radix(dev_info[0], 16).unwrap_or(0);
            let tmp_min = u32::from_str_radix(dev_info[1], 16).unwrap_or(0);
            let tmp_inode = parts[4].parse::<u64>().unwrap_or(0);
            let tmp_device = (tmp_maj as u64) * 256 + (tmp_min as u64);
            for device in device_list.iter() {
                if device.device_id == tmp_device {
                    add_process(names, pid, uid, access.clone(), ProcType::Normal, None);
                }
            }
            for inode in inode_list.iter() {
                if inode.device_id == tmp_device && inode.inode == tmp_inode {
                    add_process(names, pid, uid, access.clone(), ProcType::Normal, None);
                }
            }
        }
    }

    Ok(())
}

/// get stat of current /proc/{pid}/{filename}
fn get_pid_stat(pid: i32, filename: &str) -> Result<libc::stat, std::io::Error> {
    let path = format!("{}/{}{}", PROC_PATH, pid, filename);
    timeout(&path, 5)
}

fn fill_unix_cache(unix_socket_list: &mut UnixSocketList) -> Result<(), std::io::Error> {
    let file = File::open("/proc/net/unix")?;
    let reader = io::BufReader::new(file);

    for line in reader.lines() {
        let line = line?;

        let parts: Vec<&str> = line.split_whitespace().collect();
        let net_inode = match parts.get(6) {
            Some(part) => part.parse().unwrap_or(0),
            None => continue,
        };

        let scanned_path = match parts.get(7) {
            Some(part) => part.to_string(),
            None => continue,
        };

        let path = normalize_path(&scanned_path);

        let st = match timeout(&path, 5) {
            Ok(stat) => stat,
            Err(_) => continue,
        };

        UnixSocketList::add_socket(
            unix_socket_list,
            scanned_path,
            st.st_ino,
            st.st_dev,
            net_inode,
        );
    }
    Ok(())
}

fn read_proc_mounts(mount_list: &mut MountList) -> io::Result<&mut MountList> {
    let file = File::open(PROC_MOUNTS)?;
    let reader = io::BufReader::new(file);

    for line in reader.lines() {
        let line = line?;
        let parts: Vec<&str> = line.split_whitespace().collect();

        let mountpoint = PathBuf::from(parts[1].trim());
        mount_list.mountpoints.push(mountpoint);
    }

    Ok(mount_list)
}

/// Normalizes the path by removing the leading '@' if present.
fn normalize_path(scanned_path: &str) -> String {
    if scanned_path.starts_with('@') {
        scanned_path[1..].to_string()
    } else {
        scanned_path.to_string()
    }
}

fn parse_inet(
    names: &mut Names,
    ip_list: &mut IpConnections,
) -> Result<IpConnections, &'static str> {
    let mut hints: libc::addrinfo = unsafe { std::mem::zeroed() };
    hints.ai_family = PF_UNSPEC;
    let filename_str = names.filename.to_string_lossy();
    let parts: Vec<&str> = filename_str.split("/").collect();

    let protocol = parts[1];

    let hostspec = parts[0];
    let host_parts: Vec<&str> = hostspec.split(',').collect();
    let lcl_port_str = host_parts.get(0).cloned();
    let rmt_addr_str = host_parts.get(1).cloned();
    let rmt_port_str = host_parts.get(2).cloned();
    if protocol == "tcp" {
        hints.ai_socktype = SOCK_STREAM;
    } else {
        hints.ai_socktype = SOCK_DGRAM;
    }

    if rmt_addr_str.is_none() && rmt_port_str.is_none() {
        return Ok(IpConnections::new(
            names.to_owned(),
            lcl_port_str.unwrap().parse::<u64>().ok().unwrap(),
            0,
            IpAddr::V4("0.0.0.0".parse().unwrap()),
        ));
    } else {
    }
    Err("Can't parse tcp/udp net sockets")
}

fn find_net_dev() -> Result<u64, String> {
    let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
    let fd = socket.as_raw_fd();
    let mut stat_buf = unsafe { std::mem::zeroed() };
    unsafe { fstat(fd, &mut stat_buf) };
    Ok(stat_buf.st_dev as u64)
}

fn find_net_sockets(
    inode_list: &mut InodeList,
    connections_list: &IpConnections,
    protocol: &str,
    net_dev: u64,
) -> Result<InodeList, std::io::Error> {
    let pathname = format!("/proc/net/{}", protocol);

    let file = File::open(&pathname)?;
    let reader = io::BufReader::new(file);

    for line in reader.lines() {
        let line = line?;

        let mut loc_port = None;
        let mut rmt_addr = None;
        let mut rmt_port = None;
        let mut scanned_inode = None;

        let parts: Vec<&str> = line.split_whitespace().collect();

        // Parse local port
        if let Some(loc_port_str) = parts.get(1) {
            let r: Vec<&str> = loc_port_str.split(':').collect();
            if r.len() > 1 {
                loc_port = match u64::from_str_radix(r[1], 16) {
                    Ok(value) => Some(value),
                    Err(_) => None,
                };
            }
        }

        // Parse remote address
        if let Some(rmt_addr_str) = parts.get(2) {
            let r: Vec<&str> = rmt_addr_str.split(':').collect();
            if r.len() > 1 {
                rmt_addr = match u64::from_str_radix(r[0], 16) {
                    Ok(value) => Some(value),
                    Err(_) => None,
                };
            }
        }

        // Parse remote port
        if let Some(rmt_port_str) = parts.get(2) {
            let r: Vec<&str> = rmt_port_str.split(':').collect();
            if r.len() > 1 {
                rmt_port = match u64::from_str_radix(r[1], 16) {
                    Ok(value) => Some(value),
                    Err(_) => None,
                };
            }
        }

        // Parse scanned inode
        if let Some(scanned_inode_str) = parts.get(9) {
            scanned_inode = match scanned_inode_str.parse::<u64>() {
                Ok(value) => Some(value),
                Err(_) => None,
            };
        }

        for connection in connections_list.iter() {
            let loc_port = loc_port.unwrap_or(0);
            let rmt_port = rmt_port.unwrap_or(0);
            let rmt_addr = rmt_addr.unwrap_or(0);
            let scanned_inode = scanned_inode.unwrap_or(0);

            if (connection.lcl_port == 0 || connection.lcl_port == loc_port)
                && (connection.rmt_port == 0 || connection.rmt_port == rmt_port)
            // && (connection.rmt_addr == rmt_addr)
            {
                return Ok(InodeList::new(
                    connection.names.clone(),
                    net_dev,
                    scanned_inode,
                ));
            }
        }
    }
    Err(std::io::Error::new(io::ErrorKind::Other, "oh no!"))
}

fn stat(filename_str: &str) -> io::Result<libc::stat> {
    let filename = CString::new(filename_str).unwrap();

    unsafe {
        let mut st: libc::stat = std::mem::zeroed();
        let rc = libc::stat(filename.as_ptr(), &mut st);
        if rc == 0 {
            Ok(st)
        } else {
            Err(io::Error::last_os_error())
        }
    }
}

fn lstat(filename_str: &str) -> io::Result<libc::stat> {
    let filename = CString::new(filename_str).unwrap();

    unsafe {
        let mut st: libc::stat = std::mem::zeroed();
        let rc = libc::lstat(filename.as_ptr(), &mut st);
        if rc == 0 {
            Ok(st)
        } else {
            Err(io::Error::last_os_error())
        }
    }
}

/// Execute stat() system call with timeout to avoid deadlock
/// on network based file systems.
fn timeout(path: &str, seconds: u32) -> Result<libc::stat, std::io::Error> {
    let (tx, rx) = mpsc::channel();

    thread::scope(|s| {
        s.spawn(|| {
            tx.send(stat(path)).unwrap();
        });
    });

    match rx.recv_timeout(Duration::from_secs(seconds.into())) {
        Ok(stat) => stat,
        Err(mpsc::RecvTimeoutError::Timeout) => Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "Operation timed out",
        )),
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            Err(io::Error::new(io::ErrorKind::Other, "Channel disconnected"))
        }
    }
}

/// This function handles relative paths by resolving them against the current working directory to absolute path
///
/// # Arguments
///
/// * `path` - [str](std::str) that represents the file path.
///
/// # Errors
///
/// Returns an error if passed invalid input.
///
/// # Returns
///
/// Returns PathBuf real_path.
pub fn expand_path(path: &str) -> Result<PathBuf, io::Error> {
    let mut real_path = if path.starts_with('/') {
        PathBuf::from("/")
    } else {
        env::current_dir()?
    };

    for component in Path::new(path).components() {
        match component {
            Component::CurDir => {
                // Ignore '.'
            }
            Component::ParentDir => {
                // Handle '..' by moving up one directory level if possible
                real_path.pop();
            }
            Component::Normal(name) => {
                // Append directory or file name
                real_path.push(name);
            }
            Component::RootDir | Component::Prefix(_) => {
                // Handle root directory or prefix
                real_path = PathBuf::from(component.as_os_str());
            }
        }
    }

    if real_path.as_os_str() != "/" && real_path.as_os_str().to_string_lossy().ends_with('/') {
        real_path.pop();
    }

    Ok(real_path)
}
