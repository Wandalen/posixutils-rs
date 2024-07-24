extern crate clap;
extern crate libc;
extern crate plib;

use clap::Parser;
use gettextrs::{bind_textdomain_codeset, setlocale, textdomain, LocaleCategory};
use plib::PROJECT_NAME;
use std::{
    env,
    ffi::CString,
    io,
    path::{Component, Path, PathBuf},
};

#[derive(Debug)]
struct Procs {
    pid: i32,
    uid: u32,
    access: i8,
    proc_type: i8,
    username: Option<i8>,
    command: Option<i8>,
    next: Option<Box<Procs>>,
}

struct Names {
    filename: PathBuf,
    name_space: u8,
    matched_procs: Vec<Procs>,
    st: libc::stat,
    next: Option<Box<Names>>,
}

impl Names {
    fn new(filename: PathBuf, name_space: u8, st: libc::stat, matched_procs: Vec<Procs>) -> Self {
        Names {
            filename,
            name_space,
            st,
            matched_procs,
            next: None,
        }
    }
}
#[derive(Debug)]
struct DeviceList {
    // name: Names
    name: PathBuf,
    device_id: u64,
    next: Option<Box<DeviceList>>,
}

impl DeviceList {
    fn new(name: PathBuf, device_id: u64) -> Self {
        DeviceList {
            name,
            device_id,
            next: None,
        }
    }

    fn add_device(&mut self, name: PathBuf, device_id: u64) {
        let new_node = Box::new(DeviceList {
            name,
            device_id,
            next: self.next.take(),
        });

        self.next = Some(new_node);
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

   
    setlocale(LocaleCategory::LcAll, "");
    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;

    let exit_code = 0;

    std::process::exit(exit_code)
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
    this_name: Names,
    device_list: &mut DeviceList,
) -> Result<(), Box<dyn std::error::Error>> {
    let device_id;
    if (libc::S_IFBLK == this_name.st.st_mode) {
        device_id = this_name.st.st_rdev;
    } else {
        device_id = this_name.st.st_dev;
    }

    DeviceList::add_device(device_list, this_name.filename.clone(), device_id);
    Ok(())
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
