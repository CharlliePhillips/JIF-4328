use std::{borrow::BorrowMut, fmt::{format, Debug}, fs::{File, OpenOptions}, io::{Read, Write}, os::{fd::AsRawFd, unix::fs::OpenOptionsExt}, process::{Command, Stdio}};
use libredox::{call::{open, read, write}, flag::{O_PATH, O_RDONLY}};  
    
fn main() {
    //https://rust-cli.github.io/book/tutorial/cli-args.html
    let arg1 = std::env::args().nth(1).expect("no arg1 given");
    let arg1l = arg1.len();
    
    let mut cmd_buf: &[u8];

    match arg1.as_str() {
        "stop" => {
            cmd_buf = b"stop";
        }

        "start" => {
            cmd_buf = b"start";
        }
        _ => {
            println!("invalid arguments arg1: {:?}", arg1);
            return;
        }
    }

    let Ok(sm_fd) = &mut OpenOptions::new().write(true)
    .open("/scheme/service-monitor") else {panic!()};

    let success = File::write(sm_fd, &cmd_buf).expect("failed to write command to service monitor");
    
    match success {
        _ => println!("write command returned value: {success:#?}")
    }

}
