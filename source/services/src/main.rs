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

        "info" => {
            let arg2 = std::env::args().nth(2).expect("no arg2 given");
            for b in format!(" {};", arg2).as_bytes() {
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
        //special case for list
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
        },

        //special case for info
        4 => {
            let mut full_info_buffer = vec![0u8; 1024]; // may be too small for this command down the line, should be dynamically sized?
            let size = File::read(sm_fd, &mut full_info_buffer).expect("failed to read info from service monitor");
            full_info_buffer.truncate(size);
            let mut data_string = match std::str::from_utf8(&full_info_buffer){
                Ok(data) => data,
                Err(e) => "<data not a valid string>"
            }.to_string();
            data_string.retain(|c| c != '\0');

            println!("{}", data_string);
        },


        _ => println!("write command returned value: {success:#?}")
    }

    

}
