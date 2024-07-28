extern crate clap;
extern crate libc;
extern crate plib;

use clap::Parser;
use core::fmt;
use gettextrs::{bind_textdomain_codeset, setlocale, textdomain, LocaleCategory};
use libc::{close, fstat, socket, AF_INET, SOCK_DGRAM};
use plib::PROJECT_NAME;
use std::{
    collections::HashMap,
    default, env,
    ffi::CString,
    fs::{self, File},
    io::{self, BufRead, Write},
    path::{Component, Path, PathBuf},
    sync::mpsc,
    thread,
    time::Duration,
};

const PROC_MOUNTS: &'static str = "/proc/mounts";
const PROC_PATH: &'static str = "/proc";

#[derive(Clone, Debug)]
enum ProcType {
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

#[derive(Clone, Debug, Default)]
enum Access {
    Cwd = 1,
    Exe = 2,
    #[default]
    File = 4,
    Root = 8,
    Mmap = 16,
    Filewr = 32,
}

#[derive(Clone, Debug, Default)]
struct Procs {
    pid: i32,
    uid: u32,
    access: Access,
    proc_type: i8,
    username: Option<i8>,
    command: Option<i8>,
    next: Option<Box<Procs>>,
}

impl Procs {
    fn new(pid: i32, uid: u32, access: Access, proc_type: ProcType) -> Self {
        Self {
            pid,
            uid,
            access,
            proc_type: proc_type as i8,
            username: None,
            command: None,
            next: None,
        }
    }
}

#[derive(Debug, Default, Clone)]
struct UnixSocketList {
    sun_name: i8,
    device_id: u64,
    inode: u64,
    net_inode: u64,
    next: Option<Box<UnixSocketList>>,
}

impl UnixSocketList {
    fn new(sun_name: i8, device_id: u64, inode: u64, net_inode: u64) -> Self {
        UnixSocketList {
            sun_name,
            device_id,
            inode,
            net_inode,
            next: None,
        }
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
}

#[derive(Debug, Clone)]
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
    users: bool,

    #[arg(required = true, name = "FILE", num_args(0..))]
    /// A pathname on which the file or file system is to be reported.
    file: Vec<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let Args {
        mount,
        named_files,
        users,
        file,
        ..
    } = args;

    let expanded_path = expand_path(&file[0])?;

    let mut names = Names::default();
    let mut device_list = DeviceList::default();
    let mut inode_list = InodeList::default();
    let unixsocket_list = UnixSocketList::default();

    names.name_space = Namespace::File as u8;
    names.filename = expanded_path;

    parse_file(&mut names, &mut inode_list)?;
    parse_unixsockets(names.clone(), &mut inode_list, unixsocket_list.clone());
    scan_procs(names.clone(), inode_list, device_list, unixsocket_list, 0)?;

    // must be implemented:
    // - mounted files

    // parse_mounts(&mut names, &mut device_list)?;

    print_matches(names, mount, named_files, users)?;

    setlocale(LocaleCategory::LcAll, "");
    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;

    let exit_code = 0;

    std::process::exit(exit_code)
}

fn print_matches(
    names: Names,
    mount: bool,
    named_files: bool,
    users: bool,
) -> Result<(), io::Error> {
    todo!();
}

fn parse_file(names: &mut Names, inode_list: &mut InodeList) -> Result<(), io::Error> {
    InodeList::add_inode(
        inode_list,
        names.to_owned(),
        names.st.inner.st_dev,
        names.st.inner.st_ino,
    );

    Ok(())
}

fn parse_unixsockets(names: Names, inode_list: &mut InodeList, unix_socket_list: UnixSocketList) {
    let net_dev = find_net_dev().unwrap();
    InodeList::add_inode(inode_list, names, net_dev, unix_socket_list.net_inode);
}

/// This function processing mount points in a filesystem
///
/// # Arguments
///
/// * `this_name` - Names that represents the names_list.
/// * `device_list` - DeviceList.
///
/// # Errors
///
/// Returns an error if passed invalid input.
///
/// # Returns
/// * `Ok(())` - If the operation completes successfully.
/// * `Err(Box<dyn std::error::Error>)` - If an error occurs during reading.
///
fn parse_mounts(
    this_name: &mut Names,
    device_list: &mut DeviceList,
) -> Result<(), Box<dyn std::error::Error>> {
    let device_id;
    if (libc::S_IFBLK == this_name.st.inner.st_mode) {
        device_id = this_name.st.inner.st_rdev;
    } else {
        device_id = this_name.st.inner.st_dev;
    }

    DeviceList::add_device(device_list, this_name.to_owned(), device_id);
    Ok(())
}

fn read_proc_mounts(mount_list: &mut Option<Box<MountList>>) -> Result<(), std::io::Error> {
    let file = File::open(PROC_MOUNTS)?;
    let reader = io::BufReader::new(file);

    for line in reader.lines() {
        let line = line?;
        let parts: Vec<&str> = line.split_whitespace().collect();
        let mountpoint = PathBuf::from(parts[1].trim());
        mount_list.as_mut().unwrap().mountpoints.push(mountpoint);
    }

    Ok(())
}

fn scan_mounts(
    names_head: Names,
    inode_list: InodeList,
    device_list: &mut DeviceList,
) -> Result<Vec<libc::stat>, std::io::Error> {
    let file = File::open(PROC_MOUNTS).expect(&format!("Cannot open {}", PROC_MOUNTS));
    let reader = io::BufReader::new(file);
    let mut contents: Vec<libc::stat> = Vec::new();
    for line in reader.lines() {
        let line = line?;
        let parts: Vec<&str> = line.split_whitespace().collect();
        let mount_point = parts[1];
        let st = match timeout(mount_point, 5) {
            Ok(stat) => stat,
            Err(_) => continue,
        };
        contents.push(st);
    }
    Ok(contents)
}

fn scan_procs(
    names_head: Names,
    inode_list: InodeList,
    device_list: DeviceList,
    unix_socket_list: UnixSocketList,
    netdev: u64,
) -> Result<(), std::io::Error> {
    for entry in fs::read_dir(PROC_PATH)? {
        let entry = entry?;
        let filename = entry.file_name().into_string().unwrap();
        if filename.parse::<i32>().is_ok() {
            let pid = filename.parse::<i32>().unwrap();
            let uid = stat(&entry.path().to_string_lossy()).unwrap().st_uid;
            let len = entry.path().to_string_lossy().len();

            // let cwd_dev = stat(&entry.path().join("cwd/").to_string_lossy()).unwrap().st_dev;
            // let exe_dev = stat(&entry.path().join("exe/").to_string_lossy()).unwrap().st_dev;
            // let root_dev = stat(&entry.path().join("root/").to_string_lossy()).unwrap().st_dev;

            let cwd_dev = get_pid_stat(pid, "/cwd").st_dev;
            let root_dev = get_pid_stat(pid, "/root").st_dev;

            for device in device_list.iter() {
                if root_dev == device.device_id {
                    add_matched_proc(&mut device.name.clone(), pid, uid, Access::Root);
                }
                if cwd_dev == device.device_id {
                    add_matched_proc(&mut device.name.clone(), pid, uid, Access::Cwd);
                }
            }
        }
    }
    Ok(())
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

fn add_matched_proc(names: &mut Names, pid: i32, uid: u32, access: Access) {
    let proc = Procs::new(pid, uid, access, ProcType::Normal);
    names.matched_procs.push(proc);
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

/// get stat of current /proc/{pid}/{filename}
fn get_pid_stat(pid: i32, filename: &str) -> libc::stat {
    let path = format!("{}/{}{}", PROC_PATH, pid, filename);
    timeout(&path, 5).unwrap()
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
