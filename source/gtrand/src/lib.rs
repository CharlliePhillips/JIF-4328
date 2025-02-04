pub struct ManagedScheme {
    // these bytes will hold data to be read through the scheme this is attached to
    bytes: [u8; 32],
    // set to true when a request has been written and the scheme is waiting for the response to be read
    pub response_pending: bool,
    pid: usize,
}
impl ManagedScheme {
    //constructor
    pub fn new() -> ManagedScheme {
        ManagedScheme {
            bytes: [0;32],
            response_pending: false,
            pid: std::process::id().try_into().unwrap(),
        }
    }

    // match the request on the buffer to 
    pub fn handle_sm_request(&mut self, buf: &[u8]) -> bool {
        self.response_pending = true;
        match buf {
            b"pid" => {
                self.pid();
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
        for b in self.bytes {
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
            self.bytes[i] = b;
            i += 1;
        }
    }
}