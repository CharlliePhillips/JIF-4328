use std::os::fd;
use std::{borrow::BorrowMut, fmt::{format, Debug}, fs::{File, OpenOptions}, io::{Read, Write}, os::{fd::AsRawFd, unix::fs::OpenOptionsExt}, process::{Command, Stdio}};
use libredox::{call::{open, read, write}, flag::{O_PATH, O_RDONLY}};
use log::info;
use redox_scheme::Scheme;
use syscall::{error::*, MODE_CHR};

//use std::fs::File;
// Ty is to leave room for other types of monitor schemes
// maybe an int or enum for the command, string buffer for service name?

// todo: replace cmd u32 in SMScheme with enum we can reuse and better define
// pub enum Cmd {
//     Start(String),
//     Stop(String),
//     List
// }


pub struct SMScheme {
    pub cmd: u32, 
    pub arg1: String,
    pub pid_buffer: Vec<u8>, //used in list, could be better as the BTreeMap from service-monitor later?
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
        //if self.cmd == 3 {
        //  for each 8 bytes in buf:
        //      buf[8 bytes] = pid; (usize = 64 bits or 8 bytes)
        //  Ok(buf.length())
        //}
        // in services/main.rs smth like
        // Ok(buffer_size) = read(sm_fd, pid_buffer)
        // for each 8 bytes in pid_buffer print as usize;
        //Ok(0)

        //info!("Read called with cmd: {}", self.cmd);
        if self.cmd == 3 {
            let size = std::cmp::min(buf.len(), self.pid_buffer.len());
            buf[..size].copy_from_slice(&self.pid_buffer[..size]);
            info!("Read {} bytes from pid_buffer: {:?}", size, &buf[..size]);
            self.cmd = 0; //unlike the other commands, needs to fix cmd here instead of in main
            Ok(size)
        } else {
            Ok(0)
        } 
    }



    fn write(&mut self, _file: usize, buffer: &[u8], _offset: u64, _flags: u32) -> Result<usize> {
        //if buf contains "stop" set command = 1
        let mut r = 0;
        //println!("service-monitor command buffer: {buffer:#?}");

        match &buffer[0..5] {
            b"stop " => {
                self.cmd = 1;
                let mut idx: usize = 5;
                while(buffer[idx] != b';') {
                    self.arg1.push(buffer[idx] as char);
                    idx += 1;
                }
                
                r = 1;
            }

            b"start" => {
                self.cmd = 2;
                let mut idx: usize = 6;
                while(buffer[idx] != b';') {
                    self.arg1.push(buffer[idx] as char);
                    idx += 1;
                }
                r = 2;
            }

            b"list " => {
                self.cmd = 3;
                r = 3;
            }


            _ => {
                self.cmd = 0;
                r = 0;
            }
        }

        Ok(r)       
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
