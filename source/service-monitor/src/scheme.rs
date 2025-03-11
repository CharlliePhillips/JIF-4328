use libredox::{
    call::{open, read, write},
    flag::{O_PATH, O_RDONLY},
};
use log::info;
use redox_scheme::Scheme;
use shared::{RegistryCommand, SMCommand};
use std::os::fd;
use std::{
    borrow::BorrowMut,
    fmt::{format, Debug},
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::{fd::AsRawFd, unix::fs::OpenOptionsExt},
    process::{Command, Stdio},
};
use syscall::{error::*, MODE_CHR};

//use std::fs::File;
// Ty is to leave room for other types of monitor schemes
// maybe an int or enum for the command, string buffer for service name?

pub struct SMScheme {
    pub cmd: Option<SMCommand>,
    pub response_buffer: Vec<u8>,
}

impl SMScheme {
    //temp for getting some stuff to work
    fn read_buffer(&mut self, buf: &mut [u8]) -> Result<usize> {
        let size = std::cmp::min(buf.len(), self.response_buffer.len());
        buf[..size].copy_from_slice(&self.response_buffer[..size]);
        //info!("Read {} bytes from info_buffer: {:?}", size, &buf[..size]);
        self.cmd = None; // unlike the other commands, needs to fix cmd here instead of in main
        Ok(size)
    }
}

impl Scheme for SMScheme {
    fn open(&mut self, _path: &str, _flags: usize, _uid: u32, _gid: u32) -> Result<usize> {
        Ok(0)
    }

    fn dup(&mut self, _file: usize, buf: &[u8]) -> Result<usize> {
        if !buf.is_empty() {
            return Err(Error::new(EINVAL));
        }

        Ok(0)
    }

    fn read(&mut self, _file: usize, buf: &mut [u8], _offset: u64, _flags: u32) -> Result<usize> {
        match &self.cmd {
            Some(SMCommand::List) | Some(SMCommand::Info { .. }) => {
                return self.read_buffer(buf);
            }
            Some(SMCommand::Registry { subcommand }) => match subcommand {
                RegistryCommand::View { .. } => {
                    return self.read_buffer(buf);
                }
                _ => Ok(0),
            },
            _ => Ok(0),
        }
    }

    fn write(&mut self, _file: usize, buffer: &[u8], _offset: u64, _flags: u32) -> Result<usize> {
        self.cmd = match SMCommand::decode(buffer) {
            Ok(cmd) => Some(cmd),
            Err(_) => None,
        };
        Ok(0)
    }

    fn fcntl(&mut self, _id: usize, _cmd: usize, _arg: usize) -> Result<usize> {
        Ok(0)
    }
    fn fsize(&mut self, _id: usize) -> Result<u64> {
        Ok(0)
    }
    fn ftruncate(&mut self, _id: usize, _len: usize) -> Result<usize> {
        Ok(0)
    }

    fn fpath(&mut self, _id: usize, buf: &mut [u8]) -> Result<usize> {
        let scheme_path = b"/scheme/service-monitor";
        let size = std::cmp::min(buf.len(), scheme_path.len());

        buf[..size].copy_from_slice(&scheme_path[..size]);

        Ok(size)
    }

    fn fsync(&mut self, _file: usize) -> Result<usize> {
        Ok(0)
    }

    /// Close the file `number`
    fn close(&mut self, _file: usize) -> Result<usize> {
        Ok(0)
    }
    fn fstat(&mut self, _: usize, stat: &mut syscall::Stat) -> Result<usize> {
        stat.st_mode = 0o666 | MODE_CHR;
        stat.st_size = 0;
        stat.st_blocks = 0;
        stat.st_blksize = 4096;
        stat.st_nlink = 1;

        Ok(0)
    }
}
