use chrono::prelude::*;
use redox_scheme::{RequestKind, SchemeMut, SignalBehavior, Socket, V2};

pub struct Managment {
    // these bytes will hold data to be read through the scheme this is attached to
    response_buf: [u8; 32],
    // set to true when a request has been written and the scheme is waiting for the response to be read
    pub response_pending: bool,
    pid: usize,
    time_stamp: i64,
    message: [u8; 32],
    // [0] = read, [1] = write
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