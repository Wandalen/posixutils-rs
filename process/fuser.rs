extern crate clap;
extern crate libc;
extern crate plib;

use clap::Parser;
use core::fmt;
use gettextrs::{bind_textdomain_codeset, setlocale, textdomain, LocaleCategory};
use libc::{close, fstat, socket, AF_INET, SOCK_DGRAM};
use plib::PROJECT_NAME;
use std::{
    collections::BTreeMap,
    env,
    ffi::{CStr, CString},
    fs::{self, File},
    io::{self, BufRead},
    net::IpAddr,
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

#[derive(Clone, Debug)]
enum Namespace {
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
    name_space: u8,
    matched_procs: Vec<Procs>,
    st: LibcStat,
    next: Option<Box<Names>>,
}

impl Names {
    fn new(filename: PathBuf, name_space: u8, st: libc::stat, matched_procs: Vec<Procs>) -> Self {
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

    fn add_device(&mut self, name: Names, device_id: u64) {
        let new_node = Box::new(DeviceList {
            name,
            device_id,
            next: self.next.take(),
        });

        self.next = Some(new_node);
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
    let args = Args::parse();

    let Args {
        mount,
        named_files,
        user,
        file,
        ..
    } = args;

    // init structures
    let mut names = Names::default();
    let mut unix_socket_list = UnixSocketList::default();
    let mut mount_list = MountList::default();
    let mut device_list = DeviceList::default();
    let mut inode_list = InodeList::default();

    names.name_space = Namespace::File as u8;
    names.filename = PathBuf::from(&file[0]);

    let expanded_path = expand_path(&file[0])?;

    fill_unix_cache(&mut unix_socket_list)?;

    read_proc_mounts(&mut mount_list)?;

    let st = timeout(&names.filename.to_string_lossy(), 5)?;
    let net_dev = find_net_dev()?;

    if mount {
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


    scan_procs(
        &mut names,
        &inode_list,
        &device_list,
        &unix_socket_list,
        net_dev,
    )?;
    scan_mounts(&mut names, &inode_list, &device_list)?;

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
                _ => todo!(),
            }
        }
    }

    if !name_has_procs {
        // exit if no processes matched
        return Ok(());
    }

    let mut output = format!("{:?}:", expanded_path.display());

    for (pid, (access, uid)) in proc_map {
        let owner = if user {
            let name = unsafe {
                CStr::from_ptr(libc::getpwuid(uid).as_ref().unwrap().pw_name)
                    .to_str()
                    .unwrap()
            };
            format!("({})", name)
        } else {
            "".to_string()
        };
        output.push_str(&format!("  {}{}{}", pid, access, owner));
    }

    println!("{}", output.trim_end());

    Ok(())
}

fn scan_procs(
    names: &mut Names,
    inode_list: &InodeList,
    device_list: &DeviceList,
    unix_socket_list: &UnixSocketList,
    netdev: u64,
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

            let cwd_dev = cwd_stat.st_dev;
            let exe_dev = exe_stat.st_dev;
            let root_dev = root_stat.st_dev;

            for device in device_list.iter() {
                if root_dev == device.device_id {
                    add_matched_proc(names, pid, uid, Access::Root);
                }
                if cwd_dev == device.device_id {
                    add_matched_proc(names, pid, uid, Access::Cwd);
                }
                if exe_dev == device.device_id {
                    add_matched_proc(names, pid, uid, Access::Exe);
                }
            }

            for inode in inode_list.iter() {
                if root_dev == inode.device_id && root_stat.st_ino == inode.inode {
                    add_matched_proc(names, pid, uid, Access::Root);
                }
                if cwd_dev == inode.device_id && cwd_stat.st_ino == inode.inode {
                    add_matched_proc(names, pid, uid, Access::Cwd);
                }

                if exe_dev == inode.device_id && exe_stat.st_ino == inode.inode {
                    add_matched_proc(names, pid, uid, Access::Exe);
                }
            }
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

        for inode in inode_list.iter() {
            if st.st_dev == inode.device_id && st.st_ino == inode.inode {
                add_special_proc(names, ProcType::Mount, 0, find_mountp);
            }
        }

        for device in device_list.iter() {
            if st.st_dev == device.device_id {
                add_special_proc(names, ProcType::Mount, 0, find_mountp);
            }
        }
    }

    Ok(())
}

fn add_special_proc(names: &mut Names, ptype: ProcType, uid: u32, command: &str) {
    let proc = Procs::new(0, uid, Access::Mmap, ptype, String::from(command));
    names.add_procs(proc);
}

fn add_matched_proc(names: &mut Names, pid: i32, uid: u32, access: Access) {
    let proc = Procs::new(pid, uid, access, ProcType::Normal, String::new());
    names.add_procs(proc);
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


fn find_net_dev() -> io::Result<u64> {
    unsafe {
        let skt = socket(AF_INET, SOCK_DGRAM, 0);
        if skt < 0 {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Cannot open a network socket",
            ));
        }

        let mut statbuf: libc::stat = std::mem::zeroed();

        if fstat(skt, &mut statbuf) != 0 {
            let err = io::Error::last_os_error();
            close(skt);
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Cannot find socket's device number: {}", err),
            ));
        }

        Ok(statbuf.st_dev)
    }
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
