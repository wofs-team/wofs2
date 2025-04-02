//! WOFS - Write-Only File System
//! A Rust implementation of the Write-Only File System that redirects all writes to /dev/null

#![no_std]
#![feature(allocator_api)]
#![feature(static_assertions)]
#![allow(clippy::missing_safety_doc)]
#![allow(non_snake_case)]

use kernel::{
    c_str,
    file::File,
    file_operations::{FileOperations, FileOperationsVtable},
    file_system::{FileSystem, FileSystemType, FileSystemTypeTrait},
    fs::{
        filldir_t, mount::Mount, DirFlagsEmpty, FileDentrySeek, IoctlCommand, ModeType, NodeType, SeekFrom,
    },
    io_buffer::{IoBufferReader, IoBufferWriter},
    inode::{Inode, InodeOperations, InodeMode},
    miscdev::Registration,
    prelude::*,
    str::CStr,
    sync::{
        smutex::Mutex,
        Arc, ArcBorrow,
    },
    user_ptr::{UserSlicePtr, UserSlicePtrReader},
};

module! {
    type: WofsModule,
    name: "wofs",
    author: "WOFS Team (Rust)",
    description: "WOFS - Write-Only Filesystem, honoring /dev/null",
    license: "GPL v2",
    params: {
    },
}

// Constant definitions
const DEVICE_NAME: &CStr = c_str!("WOFS");
const FS_NAME: &CStr = c_str!("wofs");
const DEV_NULL_PATH: &CStr = c_str!("/dev/null");

// Global state for the module
struct WofsModule {
    _fs_registration: Pin<Box<Registration<FileSystemType>>>,
    _dev_registration: Pin<Box<Registration<WofsDevice>>>,
    dev_null_file: Mutex<Option<Pin<Box<File>>>>,
}

// Character device implementation
struct WofsDevice;

// Implementation of device operations for WofsDevice
#[vtable]
impl FileOperations for WofsDevice {
    type Data = ();

    fn open(_shared: &(), _file: &File) -> Result<Self::Data> {
        Ok(())
    }

    fn write(_data: &(), _file: &File, reader: &mut UserSlicePtrReader, _offset: u64) -> Result<usize> {
        let module = WofsModule::instance_or_init()?;
        let mut dev_null_guard = module.dev_null_file.lock();
        
        if let Some(dev_null_file) = dev_null_guard.as_mut() {
            // Read data from user and write to /dev/null
            let mut buf = vec![0u8; reader.len()].try_into_boxed_slice()?;
            let read_bytes = reader.read_slice(&mut buf)?;
            
            // Write to /dev/null (ignoring errors)
            let writer = IoBufferWriter::from_slice(&buf[..read_bytes]);
            let _ = dev_null_file.write(&writer, 0);
            
            // Always report success with full count
            Ok(read_bytes)
        } else {
            // No /dev/null file available
            pr_err!("WOFS: No /dev/null file available\n");
            Err(Error::EINVAL)
        }
    }
}

// File system implementation
struct WofsFileSystem;

#[vtable]
impl FileSystemTypeTrait for WofsFileSystem {
    fn mount(self: ArcBorrow<'_, FileSystemType>, _flags: u32, _dev_name: &CStr, _data: &[u8]) -> Result<Arc<Inode>> {
        // Create a new root inode for our filesystem
        let root_inode = Inode::new(
            ROOT_INODE_NUMBER,
            InodeMode::from_bits(ModeType::S_IFDIR as u16 | 0o755)
                .ok_or(Error::EINVAL)?,
            Arc::try_new(WofsInodeOps)?
        )?;
        
        Ok(root_inode)
    }
}

// Constants for inodes
const ROOT_INODE_NUMBER: u64 = 1;

// Inode operations for WOFS
struct WofsInodeOps;

#[vtable]
impl InodeOperations for WofsInodeOps {
    fn lookup(&self, _dir: &Inode, name: &CStr) -> Result<Arc<Inode>> {
        pr_info!("WOFS: lookup called for {:?}, creating dummy inode\n", name);
        
        // Create a new inode for whatever path is requested
        let inode = Inode::new(
            get_next_ino(),
            InodeMode::from_bits(ModeType::S_IFREG as u16 | 0o644)
                .ok_or(Error::EINVAL)?,
            Arc::try_new(WofsFileOps)?
        )?;
        
        Ok(inode)
    }
    
    fn mkdir(&self, _dir: &Inode, _name: &CStr, _mode: InodeMode) -> Result {
        pr_info!("WOFS: mkdir called (ignored but succeeds)\n");
        Ok(())
    }
    
    fn create(&self, _dir: &Inode, name: &CStr, _mode: InodeMode, _exclusive: bool) -> Result<Arc<Inode>> {
        pr_info!("WOFS: create called for {:?} (ignored but succeeds)\n", name);
        
        // Just create a fake inode
        let inode = Inode::new(
            get_next_ino(),
            InodeMode::from_bits(ModeType::S_IFREG as u16 | 0o644)
                .ok_or(Error::EINVAL)?,
            Arc::try_new(WofsFileOps)?
        )?;
        
        Ok(inode)
    }
}

// File operations for WOFS files
struct WofsFileOps;

#[vtable]
impl FileOperations for WofsFileOps {
    type Data = ();

    fn open(_shared: &(), _file: &File) -> Result<Self::Data> {
        Ok(())
    }

    fn write(_data: &(), _file: &File, reader: &mut UserSlicePtrReader, _offset: u64) -> Result<usize> {
        let module = WofsModule::instance_or_init()?;
        let mut dev_null_guard = module.dev_null_file.lock();
        
        if let Some(dev_null_file) = dev_null_guard.as_mut() {
            // Read data from user and write to /dev/null
            let mut buf = vec![0u8; reader.len()].try_into_boxed_slice()?;
            let read_bytes = reader.read_slice(&mut buf)?;
            
            // Write to /dev/null (ignoring errors)
            let writer = IoBufferWriter::from_slice(&buf[..read_bytes]);
            let _ = dev_null_file.write(&writer, 0);
            
            // Always report success with full count
            Ok(read_bytes)
        } else {
            // No /dev/null file available
            pr_err!("WOFS: No /dev/null file available\n");
            Err(Error::EINVAL)
        }
    }
}

// Helper function to get next inode number
fn get_next_ino() -> u64 {
    // In a full implementation, you'd use atomic operations to generate unique IDs
    // For simplicity, we return a random high number
    2 + kernel::random::get_random_u32() as u64
}

// Implementation of the kernel module
impl kernel::Module for WofsModule {
    fn init(name: &'static CStr, _module: &'static ThisModule) -> Result<Self> {
        pr_info!("WOFS: Initializing module\n");
        
        // Open /dev/null first
        let dev_null_file = File::open(DEV_NULL_PATH, 0, ModeType::O_WRONLY as _)?;
        
        // Register filesystem type
        let fs_registration = Registration::new_pinned(FileSystemType::new(
            FS_NAME,
            FileSystemType::DEFAULT_FLAGS,
            Arc::try_new(WofsFileSystem)?
        )?)?;
        
        // Register character device
        let dev_registration = Registration::new_pinned(
            DEVICE_NAME,
            Arc::try_new(WofsDevice)?
        )?;
        
        pr_info!("WOFS: Initialized successfully, all writes redirected to /dev/null\n");
        
        Ok(Self {
            _fs_registration: fs_registration,
            _dev_registration: dev_registration,
            dev_null_file: Mutex::new(Some(dev_null_file)),
        })
    }
}

impl Drop for WofsModule {
    fn drop(&mut self) {
        pr_info!("WOFS: Unloading module, ensuring cleanup\n");
        
        // Close /dev/null reference safely
        let mut dev_null_guard = self.dev_null_file.lock();
        *dev_null_guard = None;
        
        pr_info!("WOFS: Unloaded successfully\n");
    }
}
