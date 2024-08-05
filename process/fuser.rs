extern crate clap;
extern crate libc;
extern crate plib;

use clap::Parser;
use core::fmt;
use gettextrs::{bind_textdomain_codeset, setlocale, textdomain, LocaleCategory};
use libc::{fstat, SOCK_DGRAM, SOCK_STREAM};
use plib::PROJECT_NAME;
use std::{
    collections::BTreeMap,
    env,
    ffi::{CStr, CString},
    fs::{self, File},
    io::{self, BufRead, Error, ErrorKind, Write},
    net::{IpAddr, UdpSocket},
    os::unix::io::AsRawFd,
    path::{Component, Path, PathBuf},
    sync::mpsc,
    thread,
    time::Duration,
};

const PROC_PATH: &'static str = "/proc";
const PROC_MOUNTS: &'static str = "/proc/mounts";
const NAME_FIELD: usize = 20;

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
    None = 0,
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
        }
    }

    fn add_procs(&mut self, proc: Procs) {
        self.matched_procs.push(proc);
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
    file: Vec<PathBuf>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    setlocale(LocaleCategory::LcAll, "");
    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;

    let Args {
        mount, user, file, ..
    } = Args::parse();

    let (
        mut names,
        mut unix_socket_list,
        mut mount_list,
        mut device_list,
        mut inode_list,
        mut tcp_connection_list,
        mut udp_connection_list,
    ) = init_defaults(file);

    fill_unix_cache(&mut unix_socket_list)?;

    let net_dev = find_net_dev()?;

    for name in names.iter_mut() {
        name.name_space = determine_namespace(&name.filename);

        match name.name_space {
            NameSpace::File => handle_file_namespace(
                name,
                mount,
                &mut mount_list,
                &mut inode_list,
                &mut device_list,
            )?,
            NameSpace::Tcp => {
                handle_tcp_namespace(name, &mut tcp_connection_list, &mut inode_list, net_dev)?
            }
            NameSpace::Udp => {
                handle_udp_namespace(name, &mut udp_connection_list, &mut inode_list, net_dev)?
            }
        }

        scan_procs(
            name,
            &inode_list,
            &mut mount_list,
            &device_list,
            &unix_socket_list,
            net_dev,
        )?;

        print_matches(name, user)?;
    }
    let exit_code = 0;

    std::process::exit(exit_code)
}

fn init_defaults(
    file: Vec<PathBuf>,
) -> (
    Vec<Names>,
    UnixSocketList,
    MountList,
    DeviceList,
    InodeList,
    IpConnections,
    IpConnections,
) {
    let mut names = Names::default();

    names.filename = file[0].clone();
    let mut names_vec = vec![];
    for name in file.iter() {
        names_vec.push(Names::new(
            name.clone(),
            NameSpace::default(),
            unsafe { std::mem::zeroed() },
            vec![],
        ))
    }

    let unix_socket_list = UnixSocketList::default();
    let mount_list = MountList::default();
    let device_list = DeviceList::default();
    let inode_list = InodeList::default();
    let tcp_connection_list = IpConnections::default();
    let udp_connection_list = IpConnections::default();

    (
        names_vec,
        unix_socket_list,
        mount_list,
        device_list,
        inode_list,
        tcp_connection_list,
        udp_connection_list,
    )
}

fn determine_namespace(path: &PathBuf) -> NameSpace {
    if let Some(name_str) = path.to_str() {
        if name_str.contains("tcp") {
            NameSpace::Tcp
        } else if name_str.contains("udp") {
            NameSpace::Udp
        } else if name_str.contains("file") {
            NameSpace::File
        } else {
            NameSpace::default()
        }
    } else {
        NameSpace::default()
    }
}

fn handle_file_namespace(
    names: &mut Names,
    mount: bool,
    mount_list: &mut MountList,
    inode_list: &mut InodeList,
    device_list: &mut DeviceList,
) -> Result<(), std::io::Error> {
    names.filename = expand_path(&names.filename)?;
    let st = timeout(&names.filename.to_string_lossy(), 5)?;

    if mount {
        read_proc_mounts(mount_list)?;
        *device_list = DeviceList::new(names.clone(), st.st_dev);
    } else {
        let st = stat(&names.filename.to_string_lossy())?;
        *inode_list = InodeList::new(names.clone(), st.st_dev, st.st_ino);
    }
    Ok(())
}

fn handle_tcp_namespace(
    names: &mut Names,
    tcp_connection_list: &mut IpConnections,
    inode_list: &mut InodeList,
    net_dev: u64,
) -> Result<(), std::io::Error> {
    let tcp_connection_list = parse_inet(names, tcp_connection_list).unwrap();
    *inode_list = find_net_sockets(inode_list, &tcp_connection_list, "tcp", net_dev)?;
    Ok(())
}

fn handle_udp_namespace(
    names: &mut Names,
    udp_connection_list: &mut IpConnections,
    inode_list: &mut InodeList,
    net_dev: u64,
) -> Result<(), std::io::Error> {
    let udp_connection_list = parse_inet(names, udp_connection_list).unwrap();
    *inode_list = find_net_sockets(inode_list, &udp_connection_list, "udp", net_dev)?;
    Ok(())
}

fn parse_mounts(names: Names, st: libc::stat) -> DeviceList {
    let device_id;
    if libc::S_IFBLK == names.st.inner.st_mode {
        device_id = st.st_rdev;
    } else {
        device_id = st.st_dev;
    }

    DeviceList::new(names.clone(), device_id)
}

fn print_matches(name: &mut Names, user: bool) -> Result<(), io::Error> {
    let mut proc_map: BTreeMap<i32, (String, u32)> = BTreeMap::new();
    let mut name_has_procs = false;
    let mut len = name.filename.to_string_lossy().len() + 1;

    eprint!("{}:", name.filename.display());
    while len < NAME_FIELD {
        len += 1;
        eprint!(" ");
    }
    io::stderr().flush()?;

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
            _ => (),
        }
    }

    if !name_has_procs {
        // exit if no processes matched
        return Ok(());
    }

    for (pid, (access, uid)) in proc_map {
        let width = if pid.to_string().len() > 4 { " " } else { "  " };

        print!("{}{}", width, pid);
        io::stdout().flush()?;

        eprint!("{}", access);
        if user {
            let owner = unsafe {
                CStr::from_ptr(libc::getpwuid(uid).as_ref().unwrap().pw_name)
                    .to_str()
                    .unwrap()
            };
            eprint!("({})", owner);
        }
        io::stderr().flush()?;
    }

    eprint!("\n");
    Ok(())
}

/// Scans the `/proc` directory for process information and checks various access types.
fn scan_procs(
    names: &mut Names,
    inode_list: &InodeList,
    mount_list: &mut MountList,
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

        // Parse the filename as a process ID
        if let Ok(pid) = filename.parse::<i32>() {
            // Skip the current process
            if pid == my_pid {
                continue;
            }

            // Get file status for /cwd, /exe, and /root
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

            let st = timeout(&entry.path().to_string_lossy(), 5)?;
            let uid = st.st_uid;

            check_root_access(names, pid, uid, &root_stat, device_list, inode_list)?;
            check_cwd_access(names, pid, uid, &cwd_stat, device_list, inode_list)?;
            check_exe_access(names, pid, uid, &exe_stat, device_list, inode_list)?;

            #[cfg(target_os = "linux")]
            {
                check_dir(
                    names,
                    pid,
                    "lib",
                    device_list,
                    inode_list,
                    mount_list,
                    uid,
                    Access::Mmap,
                    unix_socket_list,
                    net_dev,
                )?;

                check_dir(
                    names,
                    pid,
                    "mmap",
                    device_list,
                    inode_list,
                    mount_list,
                    uid,
                    Access::Mmap,
                    unix_socket_list,
                    net_dev,
                )?;
            }
            check_dir(
                names,
                pid,
                "fd",
                device_list,
                inode_list,
                mount_list,
                uid,
                Access::File,
                unix_socket_list,
                net_dev,
            )?;

            // check_map(
            //     names,
            //     pid,
            //     "maps",
            //     device_list,
            //     inode_list,
            //     uid,
            //     Access::Mmap,
            // )?;
        }
    }

    Ok(())
}

fn is_mountpoint(mount_list: &mut MountList, path: &str) -> bool {
    if path.is_empty() {
        return false;
    }

    let trimmed_path = PathBuf::from(path.trim_end_matches('/'));

    mount_list
        .mountpoints
        .iter()
        .any(|mount| *mount == trimmed_path)
}

fn check_root_access(
    names: &mut Names,
    pid: i32,
    uid: u32,
    root_stat: &libc::stat,
    device_list: &DeviceList,
    inode_list: &InodeList,
) -> Result<(), io::Error> {
    if device_list
        .iter()
        .any(|device| device.device_id == root_stat.st_dev)
    {
        add_process(names, pid, uid, Access::Root, ProcType::Normal, None);
        return Ok(());
    }
    if inode_list
        .iter()
        .any(|inode| inode.device_id == root_stat.st_dev && inode.inode == root_stat.st_ino)
    {
        add_process(names, pid, uid, Access::Root, ProcType::Normal, None);
        return Ok(());
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
    if device_list
        .iter()
        .any(|device| device.device_id == cwd_stat.st_dev)
    {
        add_process(names, pid, uid, Access::Cwd, ProcType::Normal, None);
        return Ok(());
    }
    if inode_list
        .iter()
        .any(|inode| inode.device_id == cwd_stat.st_dev && inode.inode == cwd_stat.st_ino)
    {
        add_process(names, pid, uid, Access::Cwd, ProcType::Normal, None);
        return Ok(());
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
) -> Result<(), io::Error> {
    if device_list
        .iter()
        .any(|device| device.device_id == exe_stat.st_dev)
    {
        add_process(names, pid, uid, Access::Exe, ProcType::Normal, None);
        return Ok(());
    }
    if inode_list
        .iter()
        .any(|inode| inode.device_id == exe_stat.st_dev && inode.inode == exe_stat.st_ino)
    {
        add_process(names, pid, uid, Access::Exe, ProcType::Normal, None);
        return Ok(());
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
    mount_list: &mut MountList,
    uid: u32,
    access: Access,
    unix_socket_list: &UnixSocketList,
    net_dev: u64,
) -> Result<(), io::Error> {
    let dir_path = format!("/proc/{}/{}", pid, dirname);

    if let Ok(dir_entries) = fs::read_dir(&dir_path) {
        for entry in dir_entries {
            let entry = entry?;
            let path = entry.path();
            let path_str = path.to_string_lossy();

            let mut stat = match timeout(&path_str, 5) {
                Ok(stat) => stat,
                Err(_) => continue,
            };

            if stat.st_dev == net_dev {
                if let Some(unix_socket) = unix_socket_list
                    .iter()
                    .find(|sock| sock.net_inode == stat.st_ino)
                {
                    stat.st_dev = unix_socket.device_id;
                    stat.st_ino = unix_socket.inode;
                }
            }
            if let Some(device) = device_list.iter().find(|dev| {
                dev.name.filename != PathBuf::from("")
                    && is_mountpoint(mount_list, &dev.name.filename.to_string_lossy())
                    && stat.st_dev == dev.device_id
            }) {
                add_process(
                    names,
                    pid,
                    uid,
                    match access {
                        Access::File => Access::Filewr,
                        _ => access.clone(),
                    },
                    ProcType::Normal,
                    None,
                );
            }
            if inode_list.iter().any(|inode| inode.inode == stat.st_ino) {
                add_process(
                    names,
                    pid,
                    uid,
                    match access {
                        Access::File => Access::Filewr,
                        _ => access.clone(),
                    },
                    ProcType::Normal,
                    None,
                );
            }
        }
    } else {
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
) -> Result<(), io::Error> {
    let pathname = format!("/proc/{}/{}", pid, filename);
    let file = File::open(&pathname)?;
    let reader = io::BufReader::new(file);

    for line in reader.lines() {
        let line = line?;
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 5 {
            let device_str = parts[3];
            let inode_str = parts[4];

            let dev_info: Vec<&str> = parts[3].split(':').collect();
            let tmp_inode = match parts[4].parse::<u64>() {
                Ok(value) => value,
                Err(_) => continue,
            };
            if dev_info.len() == 2 {
                let tmp_maj = match u32::from_str_radix(dev_info[0], 16) {
                    Ok(value) => value,
                    Err(_) => continue,
                };

                let tmp_min = match u32::from_str_radix(dev_info[1], 16) {
                    Ok(value) => value,
                    Err(_) => continue,
                };

                let device = tmp_maj * 256 + tmp_min;
                let device_u64 = device as u64;

                if device_list
                    .iter()
                    .any(|device| device.device_id == device_u64)
                {
                    add_process(names, pid, uid, access.clone(), ProcType::Normal, None);
                }

                //     if inode_list
                //         .iter()
                //         .any(|inode| inode.device_id == tmp_device_u64 && inode.inode == tmp_inode)
                //     {
                //         add_process(names, pid, uid, access.clone(), ProcType::Normal, None);
                //     }
            }
        }
    }
    Ok(())
}

/// get stat of current /proc/{pid}/{filename}
fn get_pid_stat(pid: i32, filename: &str) -> Result<libc::stat, io::Error> {
    let path = format!("{}/{}{}", PROC_PATH, pid, filename);
    timeout(&path, 5)
}

/// Fills the `unix_socket_list` with info from `/proc/net/unix`.
fn fill_unix_cache(unix_socket_list: &mut UnixSocketList) -> Result<(), io::Error> {
    let file = File::open("/proc/net/unix")?;
    let reader = io::BufReader::new(file);

    for line in reader.lines() {
        let line = line?;
        let parts: Vec<&str> = line.split_whitespace().collect();

        if let (Some(net_inode_str), Some(scanned_path)) = (parts.get(6), parts.get(7)) {
            let net_inode = net_inode_str.parse().unwrap_or(0);
            let path = normalize_path(scanned_path);

            match timeout(&path, 5) {
                Ok(stat) => UnixSocketList::add_socket(
                    unix_socket_list,
                    scanned_path.to_string(),
                    stat.st_dev,
                    stat.st_ino,
                    net_inode,
                ),
                Err(_) => continue,
            }
        }
    }
    Ok(())
}

/// Reads the `/proc/mounts` file and populates the provided `MountList` with mount points.
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

/// Normalizes a file path by removing the leading '@' character if present.
fn normalize_path(scanned_path: &str) -> String {
    if scanned_path.starts_with('@') {
        scanned_path[1..].to_string()
    } else {
        scanned_path.to_string()
    }
}

/// Parses network socket information from the `filename` field of the `Names` struct
/// and returns an `IpConnections` instance.
fn parse_inet(names: &mut Names, ip_list: &mut IpConnections) -> io::Result<IpConnections> {
    let filename_str = names.filename.to_string_lossy();
    let parts: Vec<&str> = filename_str.split('/').collect();

    if parts.len() < 2 {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "Invalid filename format",
        ));
    }

    let protocol = parts[1];
    let hostspec = parts[0];
    let host_parts: Vec<&str> = hostspec.split(',').collect();

    let lcl_port_str = host_parts
        .get(0)
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "Local port is missing"))?;
    let rmt_addr_str = host_parts.get(1).cloned();
    let rmt_port_str = host_parts.get(2).cloned();

    let lcl_port = lcl_port_str
        .parse::<u64>()
        .map_err(|_| Error::new(ErrorKind::InvalidInput, "Invalid local port format"))?;

    let protocol_socktype = match protocol {
        "tcp" => libc::SOCK_STREAM,
        "udp" => libc::SOCK_DGRAM,
        _ => {
            return Err(Error::new(ErrorKind::InvalidInput, "Unsupported protocol"));
        }
    };

    if rmt_addr_str.is_none() && rmt_port_str.is_none() {
        let rmt_addr = IpAddr::V4("0.0.0.0".parse().unwrap());

        Ok(IpConnections::new(names.clone(), lcl_port, 0, rmt_addr))
    } else {
        Err(Error::new(
            ErrorKind::InvalidInput,
            "Can't parse tcp/udp net socket",
        ))
    }
}

/// Retrieves the device identifier of the network interface associated with a UDP socket.
fn find_net_dev() -> Result<u64, io::Error> {
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    let fd = socket.as_raw_fd();
    let mut stat_buf = unsafe { std::mem::zeroed() };

    unsafe {
        if fstat(fd, &mut stat_buf) != 0 {
            return Err(io::Error::last_os_error());
        }
    }

    Ok(stat_buf.st_dev as u64)
}

/// Finds network sockets based on the given protocol and updates the `InodeList`
/// with the relevant inode information if a matching connection is found.
///
/// # Arguments
///
/// * `inode_list` - A mutable reference to the `InodeList` that will be updated.
/// * `connections_list` - A reference to the `IpConnections` that will be used to match connections.
/// * `protocol` - A `&str` representing the protocol (e.g., "tcp", "udp") to look for in `/proc/net`.
/// * `net_dev` - A `u64` representing the network device identifier.
///
/// # Errors
///
/// Returns an `io::Error` if there is an issue opening or reading the file at `/proc/net/{protocol}`, or
/// if parsing the net sockets fails.
///
/// # Returns
///
/// Returns an `InodeList` containing the updated information if a matching connection is found.
/// Returns an `io::Error` with `ErrorKind::ConnectionRefused` if can't parse sockets.

fn find_net_sockets(
    inode_list: &mut InodeList,
    connections_list: &IpConnections,
    protocol: &str,
    net_dev: u64,
) -> Result<InodeList, io::Error> {
    let pathname = format!("/proc/net/{}", protocol);

    let file = File::open(&pathname)?;
    let reader = io::BufReader::new(file);

    for line in reader.lines() {
        let line = line?;

        let parts: Vec<&str> = line.split_whitespace().collect();
        let parse_hex_port = |port_str: &str| -> Option<u64> {
            port_str
                .split(':')
                .nth(1)
                .and_then(|s| u64::from_str_radix(s, 16).ok())
        };

        let loc_port = parts.get(1).and_then(|&s| parse_hex_port(s));
        let rmt_addr = parts
            .get(2)
            .and_then(|&s| parse_hex_port(s.split(':').next().unwrap_or("")));
        let rmt_port = parts.get(2).and_then(|&s| parse_hex_port(s));
        let scanned_inode = parts.get(9).and_then(|&s| s.parse::<u64>().ok());

        if let Some(scanned_inode) = scanned_inode {
            for connection in connections_list.iter() {
                let loc_port = loc_port.unwrap_or(0);
                let rmt_port = rmt_port.unwrap_or(0);
                let rmt_addr = rmt_addr.unwrap_or(0);

                if (connection.lcl_port == 0 || connection.lcl_port == loc_port)
                    && (connection.rmt_port == 0 || connection.rmt_port == rmt_port)
                {
                    return Ok(InodeList::new(
                        connection.names.clone(),
                        net_dev,
                        scanned_inode,
                    ));
                }
            }
        }
    }

    Err(Error::new(
        ErrorKind::ConnectionRefused,
        "Cannot parse net sockets",
    ))
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

/// Execute stat() system call with timeout to avoid deadlock
/// on network based file systems.
fn timeout(path: &str, seconds: u32) -> Result<libc::stat, io::Error> {
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
pub fn expand_path(path: &PathBuf) -> Result<PathBuf, io::Error> {
    let mut real_path = if path.starts_with(Path::new("/")) {
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
