use chrono::prelude::*;
use redox_scheme::{RequestKind, Scheme, SignalBehavior, Socket, CallerCtx, OpenResult};
use syscall::data::Stat;
use syscall::flag::EventFlags;
use syscall::{
    Error, Result, EBADF, EBADFD, EEXIST, EINVAL, ENOENT, EPERM, MODE_CHR, O_CLOEXEC, O_CREAT,
    O_EXCL, O_RDONLY, O_RDWR, O_STAT, O_WRONLY, SchemeMut
};

use zerocopy::IntoBytes;
use std::num::Wrapping;
use std::sync::*;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use hashbrown::HashMap;
use chrono::Local;
use std::ops::Deref;


type managementSubScheme = Arc<Mutex<Box<dyn ManagedScheme>>>;
type SubSchemeGuard<'a> = MutexGuard<'a, Box<dyn ManagedScheme>>;

struct PidScheme(u64);
struct RequestsScheme{
    reads: u64, 
    writes: u64,
    opens: u64,
    closes: u64,
    dups: u64,
    errors: u64,
}
struct TimeStampScheme(i64);
struct MessageScheme([u8;32]);
// will hold a command enum?
struct ControlScheme{
    stop: bool,
    clear: bool,
}

pub struct BaseScheme {
    main_scheme: managementSubScheme,
    pid_scheme: managementSubScheme,
    requests_scheme: managementSubScheme,
    time_stamp_scheme: managementSubScheme,
    message_scheme: managementSubScheme,
    control_scheme: managementSubScheme,
    // handlers holds a map of the file descriptors/id to
    // the actual scheme object
    handlers: HashMap<usize, managementSubScheme>,
    next_mgmt_id: AtomicUsize,
    management: Arc<Mutex<management>>,
}

impl BaseScheme {
    pub fn new(main_scheme: impl Scheme + 'static + ManagedScheme) -> Self {
        Self {
            main_scheme: Arc::new(Mutex::new(Box::new(main_scheme))),
            pid_scheme: Arc::new(Mutex::new(Box::new(
                PidScheme(
                    std::process::id().try_into().unwrap()
                )))),
            requests_scheme: Arc::new(Mutex::new(Box::new(
                    RequestsScheme{
                        // these should all actually start at 0 but setting values for clear function testing
                        reads: 0,
                        writes: 0,
                        opens: 0,
                        closes: 0,
                        dups: 0,
                        errors: 0,
                    }
                ))),
            time_stamp_scheme: Arc::new(Mutex::new(Box::new(
                TimeStampScheme(
                    Local::now().timestamp()
                )))),
            message_scheme: Arc::new(Mutex::new(Box::new(
                MessageScheme([65; 32])))),
            control_scheme: Arc::new(Mutex::new(Box::new(
                    ControlScheme{stop: false, clear: false}
                ))),
            handlers: HashMap::new(),
            next_mgmt_id: 9999.into(),
            management: Arc::new(Mutex::new(management::new())),
        }
    }

    fn handler(&self, id: usize) -> Result<SubSchemeGuard>{
        let _update = self.update()?;
        match self.handlers.get(&id) {
            None => Err(Error::new(EBADF)),
            Some(subscheme) => subscheme.lock().map_err(|err|
                Error::new(EBADF)
            ),
        }
    }

    // need to consider what value will be returned based on what update was made?
    // for now return 1 if cleared and 0 if not, error if control scheme cannot be locked
    fn update(&self) -> Result<usize> {
        let mut control_lock = self.control_scheme.lock().map_err(|err| Error::new(EBADF))?;
        let r_buf: &mut [u8] = &mut [b'\0';2];
        // for now this id is unused but this could cause problems later
        control_lock.read(0, r_buf, 0, 0);
        // see ControlScheme fn read(), the byte at index one is our clear bit.
        if r_buf[1] == 1{
            // TODO: figure out how graceful stop affects this
            let mut message_lock = self.message_scheme.lock().map_err(|err| Error::new(EBADF))?;
            message_lock.write(0, b"message cleared", 0, 0);
            
            // TODO: get a lock on the reuqests scheme and write to clear it
            let mut requests_lock = self.requests_scheme.lock().map_err(|err| Error::new(EBADF))?;
            requests_lock.write(0, b"clear", 0, 0);

            // clear the control scheme so we know not to update again
            control_lock.write(0, b"cleared", 0, 0);
            return Ok(1);
        } else if r_buf[0] == 1{
            // graceful shutdown code could go here?
            Ok(0)
        } else {
            // this is a normal data update.
            let mut requests_lock = self.requests_scheme.lock().map_err(|err| Error::new(EBADF))?;
            let management_lock = self.management.lock().map_err(|err| Error::new(EBADF))?;
            let requests_update: &mut [u8;48] = &mut [0;48];
            for i in 0..7{
                requests_update[i] = management_lock.reads.as_bytes()[i];
                requests_update[i + 8] = management_lock.writes.as_bytes()[i];
                requests_update[i + 16] = management_lock.opens.as_bytes()[i];
                requests_update[i + 24] = management_lock.closes.as_bytes()[i];
                requests_update[i + 32] = management_lock.dups.as_bytes()[i];
                requests_update[i + 40] = management_lock.errors.as_bytes()[i];
            }
            requests_lock.write(0, requests_update, 0, 0);
            
            Ok(0)
        }
    }

    pub fn message(&self, message: &str) -> Result<[u8; 32]>{
        let mut msg_arr: &mut [u8] = &mut [0; 32];
        if (message.len() > 32){
            msg_arr.copy_from_slice(&message.as_bytes()[0..32]);
        } else {
            msg_arr[0..message.len()].copy_from_slice(&message.as_bytes());
        }
        let mut message_lock = self.message_scheme.lock().map_err(|err| Error::new(EBADF))?;
        message_lock.write(0, msg_arr, 0, 0);
        
        let old_msg: &mut [u8] = &mut [0; 32];
        message_lock.read(0, old_msg, 0, 0);
        let mut msg_out: [u8; 32] = [0; 32];
        msg_out.copy_from_slice(old_msg);
        return Ok(msg_out);
    }
}
impl Scheme for BaseScheme {
    // add ability to select subscheme from open by path?
    fn xopen(&mut self, path: &str, flags: usize,  caller: &CallerCtx) -> Result<OpenResult> {
        // get a lock on the main scheme and attempt to open it
        let mut main_lock = self.main_scheme.lock().map_err(|err| Error::new(EBADF))?;
        let open_res = main_lock.xopen(path, flags, caller); 
        // if we successfully open the main scheme and get ThisScheme{id,flags} then add a
        // new managementSubScheme to the list of handlers with that id.
        let mut management = self.management.lock().map_err(|err| Error::new(EBADF))?;
        if let Ok(OpenResult::ThisScheme{number, flags}) = open_res {
            self.handlers.insert(number, self.main_scheme.clone());
            // should we check that `count_ops()` is true?
            management.opens += 1;
            
            open_res
        } else {
            // otherwise propogate the result
            // how should errors be handled here? do we count them even if we get OpenResult::OtherScheme?
            management.errors += 1;
            open_res
        }
    }
    
    fn dup(&mut self, old_id: usize, buf: &[u8]) -> Result<usize> {
        // check if we have an existing handler for this id
        if self.handlers.contains_key(&old_id) {
            let mut result = match buf {
                // if there is a matching managementSubScheme name make a new id/handler for it
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
                    Ok(new_id)
                }

                b"request_count" => {
                    let new_id = self.next_mgmt_id.fetch_sub(1, Ordering::Relaxed);
                    self.handlers.insert(new_id, self.requests_scheme.clone());
                    Ok(new_id)
                }
                
                b"control" => {
                    let new_id = self.next_mgmt_id.fetch_sub(1, Ordering::Relaxed);
                    self.handlers.insert(new_id, self.control_scheme.clone());
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
            };
            // check to see if we want to record this dup
            let subscheme: SubSchemeGuard = self.handler(old_id)?;
            let mut management = self.management.lock().map_err(|err| Error::new(EBADF))?;
            if (!result.is_err() && subscheme.count_ops()) {
                management.dups += 1;
            } else if (subscheme.count_ops()) {
                management.errors += 1;
            }
            // return the result of the match (subscheme dup)
            return result;
        } else {
            Err(syscall::Error {errno: EBADF})
        }
    }
    
    fn read(&mut self, id: usize, buf: &mut [u8], _offset: u64, _flags: u32) -> Result<usize> {
        // lock the subscheme and management struct
        let mut subscheme: SubSchemeGuard = self.handler(id)?;
        let mut management = self.management.lock().map_err(|err| Error::new(EBADF))?;
        // read from the subscheme
        let mut result = subscheme.read(id, buf, _offset, _flags);
        // if the read did not error and its ManagedScheme impl says so, increment the read counter.
        if (!result.is_err() && subscheme.count_ops()) {
            management.reads += 1;
        } else if (subscheme.count_ops()) {
            management.errors += 1;
        }
        return result;
    }

    fn write(&mut self, id: usize, buffer: &[u8], _offset: u64, _flags: u32) -> Result<usize> {
        let mut subscheme: SubSchemeGuard = self.handler(id)?;
        let mut management = self.management.lock().map_err(|err| Error::new(EBADF))?;

        let mut result = subscheme.write(id, buffer, _offset, _flags);
        if (!result.is_err() && subscheme.count_ops()) {
            management.writes += 1;
        } else if (subscheme.count_ops()) {
            management.errors += 1;
        } 
        return result;
    }

    // TODO: unimplemented BaseScheme functions should pass to the main Scheme instead of just Oking
    fn fcntl(&mut self, id: usize, cmd: usize, arg: usize) -> Result<usize> {
        let mut main = self.main_scheme.lock()
        .map_err(|err| Error::new(EBADF))?;
                    
        main.fcntl(id, cmd, arg)
    }

    fn fsize(&mut self, id: usize) -> Result<u64> {
        let mut main = self.main_scheme.lock()
        .map_err(|err| Error::new(EBADF))?;
                    
        main.fsize(id)
    }

    fn ftruncate(&mut self, id: usize, len: usize) -> Result<usize> {
        let mut main = self.main_scheme.lock()
        .map_err(|err| Error::new(EBADF))?;
                    
        main.ftruncate(id, len)
    }

    
    fn fpath(&mut self, id: usize, buf: &mut [u8]) -> Result<usize> {
        let mut main = self.main_scheme.lock()
        .map_err(|err| Error::new(EBADF))?;
                    
        main.fpath(id, buf)
    }

    fn fsync(&mut self, id: usize) -> Result<usize> {
        let mut main = self.main_scheme.lock()
        .map_err(|err| Error::new(EBADF))?;
                    
        main.fsync(id)
    }

    fn close(&mut self, id: usize) -> Result<usize> {
        // get the scheme handler for this id
        if self.handlers.contains_key(&id) {
            // attempt to close the scheme
            let mut scheme = self.handler(id)?;
            let result = scheme.close(id);
            let mut management = self.management.lock().map_err(|err| Error::new(EBADF))?;
            if (!result.is_err() && scheme.count_ops()) {
                management.closes += 1;
            } else if (result.is_err() && scheme.count_ops()) {
                management.errors += 1;
            } 
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

    fn fstat(&mut self, id: usize, stat: &mut syscall::Stat) -> Result<usize> {
        let mut main = self.main_scheme.lock()
        .map_err(|err| Error::new(EBADF))?;
                    
        main.fstat(id, stat)
    }
}

impl ManagedScheme for PidScheme {}
impl Scheme for PidScheme {
    fn read(&mut self, _id: usize, buf: &mut [u8], _offset: u64, _flags: u32) -> Result<usize> {
        // get data as byte array
        let pid_bytes = self.0.to_ne_bytes();
        // fill passed buffer
        fill_buffer(buf, &pid_bytes);
        Ok(buf.len())
    }
}

impl ManagedScheme for RequestsScheme {}
impl Scheme for RequestsScheme {
    fn write(&mut self, _id: usize, buf: &[u8], _offset: u64, _flags: u32) -> Result<usize>{
        if buf == b"clear"{
            self.reads = 0;
            self.writes = 0;
            self.opens = 0;
            self.closes = 0;
            self.dups = 0;
            self.errors = 0;
        } else {
            let mut read_bytes: [u8; 8] = [0; 8];
            let mut write_bytes: [u8; 8] = [0; 8];
            let mut open_bytes: [u8; 8] = [0; 8];
            let mut close_bytes: [u8; 8] = [0; 8];
            let mut dup_bytes: [u8; 8] = [0; 8];
            let mut error_bytes: [u8; 8] = [0; 8];
            read_bytes.clone_from_slice(&buf[0..8]);
            write_bytes.clone_from_slice(&buf[8..16]);
            open_bytes.clone_from_slice(&buf[16..24]);
            close_bytes.clone_from_slice(&buf[24..32]);
            dup_bytes.clone_from_slice(&buf[32..40]);
            error_bytes.clone_from_slice(&buf[40..48]);
            self.reads = u64::from_ne_bytes(read_bytes);
            self.writes = u64::from_ne_bytes(write_bytes);
            self.opens = u64::from_ne_bytes(open_bytes);
            self.closes = u64::from_ne_bytes(close_bytes);
            self.dups = u64::from_ne_bytes(dup_bytes);
            self.errors = u64::from_ne_bytes(error_bytes);
        }
        Ok(buf.len())
    } 
    fn read(&mut self, _id: usize, buf: &mut [u8], _offset: u64, _flags: u32) -> Result<usize> {
        let read_bytes = &self.reads.to_ne_bytes();
        let write_bytes = &self.writes.to_ne_bytes();
        let open_bytes = &self.opens.to_ne_bytes();
        let close_bytes = &self.closes.to_ne_bytes();
        let dup_bytes = &self.dups.to_ne_bytes();
        let error_bytes = &self.errors.to_ne_bytes();
        let mut request_count_bytes: [u8; 48] = [0; 48];
        for i in 0..8 {
            request_count_bytes[i] = read_bytes[i];
            request_count_bytes[i + 8] = write_bytes[i];
            request_count_bytes[i + 16] = open_bytes[i];
            request_count_bytes[i + 24] = close_bytes[i];
            request_count_bytes[i + 32] = dup_bytes[i];
            request_count_bytes[i + 40] = error_bytes[i];
        }
        fill_buffer(buf, &request_count_bytes);
        Ok(buf.len())
    }
}

impl ManagedScheme for TimeStampScheme {}
impl Scheme for TimeStampScheme {
    fn read(&mut self, _id: usize, buf: &mut [u8], _offset: u64, _flags: u32) -> Result<usize> {
        let time_stamp = self.0.to_ne_bytes();
        
        fill_buffer(buf, &time_stamp);
        Ok(buf.len())
    }
}

impl ManagedScheme for MessageScheme {}
impl Scheme for MessageScheme {
    fn read(&mut self, _id: usize, buf: &mut [u8], _offset: u64, _flags: u32) -> Result<usize> {
        // message is already stored as an array of bytes
        fill_buffer(buf, &self.0);
        Ok(buf.len())
    }
    
    fn write(&mut self, _id: usize, buf: &[u8], _offset: u64, _flags: u32) -> Result<usize> {
        // message is already stored as an array of bytes
        self.0 = [0; 32];
        fill_buffer(&mut self.0, buf);
        Ok(buf.len())
    }
}

impl ManagedScheme for ControlScheme {}
impl Scheme for ControlScheme {
    fn read(&mut self, _id: usize, buf: &mut [u8], _offset: u64, _flags: u32) -> Result<usize> {
        // writes to the first two bytes indicating 
        buf[0] = u8::from(self.stop);
        buf[1] = u8::from(self.clear);
        Ok(buf.len())
    }
    fn write(&mut self, _id: usize, buf: &[u8], _offset: u64, _flags: u32) -> Result<usize> {
        // message is already stored as an array of bytes
        match buf {
            b"clear" => {
                self.clear = true;
            }
             
            b"cleared" => {
                self.clear = false;
            }
            
            b"stop" => {
                self.stop = true;
            }
            
            _ => {

            }
        }  
        Ok(buf.len())
    }
    fn close(&mut self, _id: usize) -> Result<usize> {
        // for some reason only this one crashes when closed using default implementation
        // this makes it not crash...?
        Ok(0)
    }
}

fn fill_buffer(dest: &mut [u8], src: &[u8]) {
    let mut i = 0;
    for byte in src {
        dest[i] = 0;
        dest[i] = *byte;
        i += 1;
    }
}

pub struct management {
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
    // request_count: (u64, u64),
    reads: u64, 
    writes: u64,
    opens: u64,
    closes: u64,
    dups: u64,
    errors: u64,

}

impl management {
    //constructor
    pub fn new() -> management {
        management {
            response_buf: [0;32],
            response_pending: false,
            pid: std::process::id().try_into().unwrap(),
            // init timestamp to unix epoch
            time_stamp: 0,
            message: [0;32],
           // request_count: (13, 42),
            reads: 0, 
            writes: 0,
            opens: 0,
            closes: 0,
            dups: 0,
            errors: 0,
        }
    }

    pub fn start_management(&mut self, message: &str) {
        self.time_stamp = Local::now().timestamp();
        let mut message_len = message.as_bytes().len();
        if message_len > 32 {
            message_len = 32
        }
        for i in 0..message_len {
            self.message[i] = message.as_bytes()[i];
        }
    }
}

pub trait ManagedScheme: Scheme {
    fn count_ops(&self) -> bool{
        return false;
    }



    fn shutdown(&mut self) -> bool {
        return false;
    }
}
