use std::{borrow::BorrowMut, fmt::{format, Debug}, fs::{File, OpenOptions}, io::{Read, Write}, os::{fd::AsRawFd, unix::fs::OpenOptionsExt}, process::{Command, Stdio}};
use libredox::{call::{open, read, write}, flag::{O_PATH, O_RDONLY}};  
    
fn main() {
    //https://rust-cli.github.io/book/tutorial/cli-args.html
    libredox::call::setrens(1, 1).expect("sm: failed to enter null namespace");
    let arg1 = std::env::args().nth(1).expect("no arg1 given");
    let arg2 = std::env::args().nth(2).expect("no arg2 given");
    let readbuf: &mut [u8] = &mut [b's', b't', b'o', b'p'];
    if (arg1 == "stop") {
        let Ok(sm_fd) = &mut OpenOptions::new().write(true).open("service-monitor_service-monitor:") else {panic!()};
            let success = File::write(sm_fd, readbuf).expect("failed to write command to service monitor");
            if success == 0 {
                println!("gtrand not stopped!");
            }
        
    }

    println!("arg1: {:?}, arg2: {:?}", arg1, arg2)
}
