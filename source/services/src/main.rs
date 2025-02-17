use std::{borrow::BorrowMut, fmt::{format, Debug}, fs::{File, OpenOptions}, io::{Read, Write}, os::{fd::AsRawFd, unix::fs::OpenOptionsExt}, process::{Command, Stdio}};
use libredox::{call::{open, read, write}, flag::{O_PATH, O_RDONLY}};  
    
// todo: replace with Clap crate for robust parsing and help-text
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

        "help" => {
            println!("Usage:
    services start <service-name>   Start service
    services stop <service-name>    Stop service
    services list                   List PIDs of currently running services");
            return;
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
        //special case for list
        // todo: figure out how to get this success code associated with the command, rather than using hardcoded number directly
        3 => {
            let mut pid_buffer = vec![0u8; 1024]; //1024 is kinda arbitrary here, may cause issues later
            let size = File::read(sm_fd, &mut pid_buffer).expect("failed to read PIDs from service monitor");
            pid_buffer.truncate(size);

            //since each PID is 4 bytes, we chunk and read that way
            let pids: Vec<usize> = pid_buffer.chunks(4).map(|chunk| {
                let mut array = [0u8; 4];
                array.copy_from_slice(chunk);
                u32::from_ne_bytes(array) as usize
            }).collect();
            println!("PIDs: {:?}", pids);
        }


        _ => println!("write command returned value: {success:#?}")
    }

    

}
