use std::{borrow::BorrowMut, fmt::{format, Debug}, fs::{File, OpenOptions}, io::{Read, Write}, os::{fd::AsRawFd, unix::fs::OpenOptionsExt}, process::{Command, Stdio}};
use libredox::{call::{open, read, write}, flag::{O_PATH, O_RDONLY}};  
    
fn main() {
    //https://rust-cli.github.io/book/tutorial/cli-args.html
    let arg1 = std::env::args().nth(1).expect("no arg1 given");
    println!("arg1: {}", arg1);
    let mut cmd_buf: Vec<u8> = arg1.clone().into_bytes();

    match arg1.as_str() {
        "stop" => {
            let arg2 = std::env::args().nth(2).expect("no arg2 given");
            for b in format!(" {};", arg2).as_bytes() {
                cmd_buf.push(*b);
            }
        }

        "start" => {
            let arg2 = std::env::args().nth(2).expect("no arg2 given");
            for b in format!(" {};", arg2).as_bytes() {
                cmd_buf.push(*b);
            }
        }

        "list" => {
            for b in format!(" ;").as_bytes() {
                cmd_buf.push(*b);
            }
            
        }

        _ => {
            println!("invalid arguments arg1: {:?}", arg1);
            return;
        }
    };

    let Ok(sm_fd) = &mut OpenOptions::new().write(true)
    .open("/scheme/service-monitor") else {panic!()};

    let success = File::write(sm_fd, &cmd_buf).expect("failed to write command to service monitor");
    
    match success {
        _ => println!("write command returned value: {success:#?}")
    }

}
