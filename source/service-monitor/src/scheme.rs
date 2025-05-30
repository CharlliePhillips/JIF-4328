use redox_scheme::Scheme;
use shared::{RegistryCommand, SMCommand, CommandResponse};
use syscall::{error::*, MODE_CHR};

//use std::fs::File;
// Ty is to leave room for other types of monitor schemes
// maybe an int or enum for the command, string buffer for service name?

pub struct SMScheme {
    pub cmd: Option<SMCommand>,
    response_buffer: Vec<u8>,
    read_index: usize,
}

impl SMScheme {
    /// Construct an [SMScheme] with default fields. 
    pub fn new() -> SMScheme {
        SMScheme {
            cmd: None,
            response_buffer: Vec::new(),
            read_index: 0,
        }
    }

    /// Write a [CommandResponse] to the response buffer.
    /// This method does not take ownership of `response`.
    pub fn write_response(&mut self, response: &CommandResponse) -> Result<usize, String> {
        toml::to_string(response)
            .map_err(|e| format!("Failed to encode SMCommand into string: {}", e))
            .map(|s| { s.into_bytes() })
            .and_then(|buf| {
                self.write_bytes(&buf)
            })
    }

    /// Write bytes to the response buffer. If the response buffer has content already in it,
    /// it will be overwritten.
    fn write_bytes(&mut self, buf: &[u8]) -> Result<usize, String> {
        self.response_buffer = buf.to_vec();
        self.read_index = 0;
        Ok(buf.len())
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
        if self.read_index != self.response_buffer.len() {
            let buf_len = buf.len();
            let res_len = self.response_buffer.len() - self.read_index;
            let size = std::cmp::min(buf_len, res_len);
            if buf_len < res_len {
                buf.copy_from_slice(
                    &self.response_buffer[self.read_index..(size + self.read_index)],
                );
            } else {
                buf[..size].copy_from_slice(&self.response_buffer[self.read_index..]);
            }
            self.read_index = self.read_index + size;
            Ok(size)
        } else {
            Ok(0)
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
