use std::os::fd;

use log::info;
use redox_scheme::SchemeMut;
use syscall::{error::*, MODE_CHR};

use crate::Ty;
// Ty is to leave room for other types of monitor schemes
// Vec is the list of currently running services
pub struct SMScheme(pub Ty, pub Vec<(usize, [u8; 32])>);

impl SchemeMut for SMScheme {

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
        Ok(0)
    }



    fn write(&mut self, _file: usize, buffer: &[u8], _offset: u64, _flags: u32) -> Result<usize> {
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
        let scheme_path = b"sm:";
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
