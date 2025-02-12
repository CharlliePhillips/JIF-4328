use chrono::prelude::*;
use redox_scheme::{RequestKind, Scheme, SignalBehavior, Socket, CallerCtx, OpenResult};
use syscall::data::Stat;
use syscall::flag::EventFlags;
use syscall::{
    Error, Result, EBADF, EBADFD, EEXIST, EINVAL, ENOENT, EPERM, MODE_CHR, O_CLOEXEC, O_CREAT,
    O_EXCL, O_RDONLY, O_RDWR, O_STAT, O_WRONLY, SchemeMut
};
use hashbrown::HashMap;
use std::num::Wrapping;
use std::sync::*;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use chrono::Local;
use std::ops::Deref;


type ManagmentSubScheme = Arc<Mutex<Box<dyn Scheme>>>;
type SubSchemeGuard<'a> = MutexGuard<'a, Box<dyn Scheme>>;

struct PidScheme(u64);
struct RequestsScheme{
    reads: u64, 
    writes: u64,
    opens: u64,
    closes: u64,
    dups: u64,
}
struct TimeStampScheme(i64);
struct MessageScheme([u8;32]);
// will hold a command enum?
struct ControlScheme{
    stop: bool,
    clear: bool,
}

pub struct BaseScheme {
    main_scheme: ManagmentSubScheme,
    pid_scheme: ManagmentSubScheme,
    requests_scheme: ManagmentSubScheme,
    time_stamp_scheme: ManagmentSubScheme,
    message_scheme: ManagmentSubScheme,
    control_scheme: ManagmentSubScheme,
    // handlers holds a map of the file descriptors/id to
    // the actual scheme object
    handlers: HashMap<usize, ManagmentSubScheme>,
    next_mgmt_id: AtomicUsize,
    managment: Managment,
}

impl BaseScheme {
    pub fn new(main_scheme: impl Scheme + 'static) -> Self {
        Self {
            main_scheme: Arc::new(Mutex::new(Box::new(main_scheme))),
            pid_scheme: Arc::new(Mutex::new(Box::new(
                PidScheme(
                    std::process::id().try_into().unwrap()
                )))),
            requests_scheme: Arc::new(Mutex::new(Box::new(
                    RequestsScheme{
                        // these should all actually start at 0 but setting values for clear function testing
                        reads: 13,
                        writes: 42,
                        opens: 32,
                        closes: 16,
                        dups: 8,
                    }
                ))),
            time_stamp_scheme: Arc::new(Mutex::new(Box::new(
                TimeStampScheme(
                    Local::now().timestamp()
                )))),
            message_scheme: Arc::new(Mutex::new(Box::new(
                MessageScheme([0; 32])))),
            control_scheme: Arc::new(Mutex::new(Box::new(
                    ControlScheme{stop: false, clear: false}
                ))),
            handlers: HashMap::new(),
            next_mgmt_id: 9999.into(),
            managment: Managment::new(),
        }
    }

    fn handler(&self, id: usize) -> Result<SubSchemeGuard>{
        match self.handlers.get(&id) {
            None => Err(Error::new(EBADF)),
            Some(subscheme) => subscheme.lock().map_err(|err|
                Error::new(EBADF)
            ),
        }
   }
}
impl Scheme for BaseScheme {
    // add ability to select subscheme from open by path?
    fn xopen(&mut self, path: &str, flags: usize,  caller: &CallerCtx) -> Result<OpenResult> {
        // get a lock on the main scheme and attempt to open it
        let mut main_lock = self.main_scheme.lock().map_err(|err| Error::new(EBADF))?;
        let open_res = main_lock.xopen(path, flags, caller); 
        // if we successfully open the main scheme and get ThisScheme{id,flags} then add a
        // new ManagmentSubScheme to the list of handlers with that id.
        if let Ok(OpenResult::ThisScheme{number, flags}) = open_res {
            self.handlers.insert(number, self.main_scheme.clone());
            open_res
        } else {
            // otherwise propogate the result
            open_res
        }
    }
    
    fn dup(&mut self, old_id: usize, buf: &[u8]) -> Result<usize> {
        // check if we have an existing handler for this id
        if self.handlers.contains_key(&old_id) {
            match buf {
                // if there is a matching ManagmentSubScheme name make a new id/handler for it
                b"pid" => {
                    let new_id = self.next_mgmt_id.fetch_sub(1, Ordering::Relaxed);
                    self.handlers.insert(new_id, self.pid_scheme.clone());
                    Ok(new_id)
                }

                b"time_stamp" => {
                    let new_id = self.next_mgmt_id.fetch_sub(1, Ordering::Relaxed);
                    self.handlers.insert(new_id, self.time_stamp_scheme.clone());
                    Ok(new_id)
                }

                b"message" => {
                    let new_id = self.next_mgmt_id.fetch_sub(1, Ordering::Relaxed);
                    self.handlers.insert(new_id, self.message_scheme.clone());
                    self.write(new_id, b"test message", 0, 0);
                    Ok(new_id)
                }

                b"request_count" => {
                    let new_id = self.next_mgmt_id.fetch_sub(1, Ordering::Relaxed);
                    self.handlers.insert(new_id, self.requests_scheme.clone());
                    Ok(new_id)
                }

                // if there is nothing on the buffer then assume we want the main scheme
                b"" => {
                    let main_dup = self.main_scheme.lock()
                    .map_err(|err| Error::new(EBADF))?
                    .dup(old_id, buf)?;
                    
                    self.handlers.insert(main_dup, self.main_scheme.clone());
                    Ok(main_dup)
                }

                // if there is something unknown on the buffer but we know the id then dup
                // the given id.
                _ => {
                    if let scheme = self.handlers.get(&old_id).ok_or(Error::new(EBADF))? {
                        let mut handler = self.handler(old_id)?;
                        let new_id = handler.dup(old_id, buf)?;
                        drop(handler);
                        self.handlers.insert(new_id, scheme.clone());
                        Ok(new_id)
                    } else {
                        // we have already checked for the key so this should never run
                        Err(syscall::Error {errno: EBADF})
                    }
               }
            }
        } else {
            Err(syscall::Error {errno: EBADF})
        }
    }
    
    fn read(&mut self, id: usize, buf: &mut [u8], _offset: u64, _flags: u32) -> Result<usize> {
        self.handler(id)?.read(id, buf, _offset, _flags)
    }

    fn write(&mut self, id: usize, buffer: &[u8], _offset: u64, _flags: u32) -> Result<usize> {
        self.handler(id)?.write(id, buffer, _offset, _flags)
    }

    // TODO: unimplemented BaseScheme functions should pass to the main Scheme instead of just Oking
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
        let mut i = 0;
        let scheme_path = b"gtrand";
        while i < buf.len() && i < scheme_path.len() {
            buf[i] = scheme_path[i];
            i += 1;
        }
        Ok(i)
    }

    fn fsync(&mut self, _file: usize) -> Result<usize> {
        Ok(0)
    }

    fn close(&mut self, id: usize) -> Result<usize> {
        // get the scheme handler for this id
        if self.handlers.contains_key(&id) {
            // attempt to close the scheme
            let mut scheme = self.handler(id)?;
            let result = scheme.close(id);
            drop(scheme);
            // we want to remove this id from the handlers map regardless close is success.
            // 'scheme' is retrieved from handlers map in 'fn handler()' so we have to drop it in
            // order to modify the map. 
            self.handlers.remove(&id);
            return result;
        } else {
            Err(syscall::Error {errno: EBADF})
        }
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

impl Scheme for PidScheme {
    fn read(&mut self, _id: usize, buf: &mut [u8], _offset: u64, _flags: u32) -> Result<usize> {
        // get data as byte array
        let pid_bytes = self.0.to_ne_bytes();
        // fill passed buffer
        fill_buffer(buf, &pid_bytes);
        Ok(buf.len())
    }
}
impl Scheme for RequestsScheme {
    fn read(&mut self, _id: usize, buf: &mut [u8], _offset: u64, _flags: u32) -> Result<usize> {
        let read_bytes = &self.reads.to_ne_bytes();
        let write_bytes = &self.writes.to_ne_bytes();
        let mut request_count_bytes: [u8; 17] = [b'\0'; 17];
        request_count_bytes[8] = b',';
        for i in 0..8 {
            request_count_bytes[i] = read_bytes[i];
            request_count_bytes[i + 9] = write_bytes[i];
        }
        fill_buffer(buf, &request_count_bytes);
        Ok(buf.len())
    }
}
impl Scheme for TimeStampScheme {
    fn read(&mut self, _id: usize, buf: &mut [u8], _offset: u64, _flags: u32) -> Result<usize> {
        let time_stamp = self.0.to_ne_bytes();
        
        fill_buffer(buf, &time_stamp);
        Ok(buf.len())
    }
}
impl Scheme for MessageScheme {
    fn read(&mut self, _id: usize, buf: &mut [u8], _offset: u64, _flags: u32) -> Result<usize> {
        // message is already stored as an array of bytes
        fill_buffer(buf, &self.0);
        Ok(buf.len())
    }
    
    fn write(&mut self, _id: usize, buf: &[u8], _offset: u64, _flags: u32) -> Result<usize> {
        // message is already stored as an array of bytes
        fill_buffer(&mut self.0, buf);
        Ok(buf.len())
    }
}

impl Scheme for ControlScheme {

}

fn fill_buffer(dest: &mut [u8], src: &[u8]) {
    let mut i = 0;
    for byte in src {
        dest[i] = *byte;
        i += 1;
    }
}

pub struct Managment {
    // these bytes will hold data to be read through the scheme this is attached to
    response_buf: [u8; 32],
    // set to true when a request has been written and the scheme is waiting for the response to be read
    pub response_pending: bool,
    // TODO ^^ probably don't need these anymore
    pid: usize,
    time_stamp: i64,
    message: [u8; 32],
    // [0] = read, [1] = write
    // TODO: this should probably get split into 5 integers for read, write, open, close, and dup
    request_count: (u64, u64),

}

impl Managment {
    //constructor
    pub fn new() -> Managment {
        Managment {
            response_buf: [0;32],
            response_pending: false,
            pid: std::process::id().try_into().unwrap(),
            // init timestamp to unix epoch
            time_stamp: 0,
            message: [0;32],
            request_count: (13, 42),
        }
    }

    pub fn start_managment(&mut self, message: &str) {
        self.time_stamp = Local::now().timestamp();
        let mut message_len = message.as_bytes().len();
        if message_len > 32 {
            message_len = 32
        }
        for i in 0..message_len {
            self.message[i] = message.as_bytes()[i];
        }
    }

    // match the request on the buffer to 
    pub fn handle_sm_request(&mut self, buf: &[u8]) -> bool {
        self.response_pending = true;
        match buf {
            b"pid" => {
                self.pid();
            }
            b"time_stamp" => {
                self.time_stamp();
            } 

            b"message" => {
                self.message();
            }

            b"request_count" => {
                self.request_count();
            }
            _ => {
                self.response_pending = false;
            }
        }
        self.response_pending
    }

    // copy the managment bytes to a buffer
    pub fn fill_buffer(&mut self, buf: &mut [u8]) {
        let mut i = 0;
        for b in self.response_buf {
            buf[i] = b;
            i += 1;
        }
        self.response_pending = false;
    }

    // moves pid into our bytes for the managment struct
    fn pid(&mut self) {
        let mut i = 0;
        println!("pid bytes: {:#?}", self.pid.to_ne_bytes());
        for b in self.pid.to_ne_bytes() {
            self.response_buf[i] = b;
            i += 1;
        }
    }

    fn time_stamp(&mut self) {
        let mut i = 0;
        println!("time stamp bytes: {:#?}", self.time_stamp.to_ne_bytes());
        for b in self.time_stamp.to_ne_bytes() {
            self.response_buf[i] = b;
            i += 1;
        }   
    }

    fn message(&mut self) {
        let mut i = 0;
        println!("message bytes: {:#?}", self.message);
        for b in self.message {
            self.response_buf[i] = b;
            i += 1;
        }
    }

    fn request_count(&mut self) {
        let mut i = 0;
        println!("read count bytes: {:#?}", self.request_count.0.to_ne_bytes());
        for b in self.request_count.0.to_ne_bytes() {
            self.response_buf[i] = b;
            i += 1;
        }
        // add a comma for the tuple
        self.response_buf[i] = b',';
        i += 1;
        println!("write count bytes: {:#?}", self.request_count.1.to_ne_bytes());
        for b in self.request_count.1.to_ne_bytes() {
            self.response_buf[i] = b;
            i += 1;
        }
    }
}

