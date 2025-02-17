use std::{str, borrow::BorrowMut, fmt::{format, Debug}, fs::{File, OpenOptions}, io::{Read, Write}, os::{fd::AsRawFd, unix::fs::OpenOptionsExt}, process::{Command, Stdio}};
use libredox::{call::{open, read, write}, flag::{O_PATH, O_RDONLY}};
use timer::Timer;
use chrono::prelude::*;

enum GenericData
{
    Byte(u8),
    Short(u16),
    Int(u32),
    Text(String)
}


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
            if let Some(arg2) = std::env::args().nth(2) {
                for b in format!(" {};", arg2).as_bytes() {
                    cmd_buf.push(*b);
                }
            } else {
                for b in format!(" ;").as_bytes() {
                    cmd_buf.push(*b);
                }
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
            let mut read_buffer = vec![0u8; 1024]; //1024 is kinda arbitrary here, may cause issues later
            let size = File::read(sm_fd, &mut read_buffer).expect("failed to read from service monitor");
            let arg2_bytes = read_buffer[size..].to_vec();
            read_buffer.truncate(size);

            //since each PID is 4 bytes, we chunk and read that way
            let pids: Vec<usize> = read_buffer.chunks(4).map(|chunk| {
                let mut array = [0u8; 4];
                array.copy_from_slice(chunk);
                u32::from_ne_bytes(array) as usize
            }).collect();
            println!("PIDs: {:?}", pids);
               
            
            
            let mut listarg2 = match str::from_utf8(&arg2_bytes) {
                Ok(listarg2) => listarg2,
                Err(e) => "<data not a valid string>"
            }.to_string();
            //change trailing 0 chars into empty string
            listarg2.retain(|c| c != '\0');


            if !listarg2.is_empty() {
                println!("arg2: {}", listarg2);
                
                let mut data_vec: Vec<GenericData> = Vec::new();
            
                match listarg2.as_str() {
                    "time_stamp" => {
                        list_helper(&listarg2, &mut data_vec);
                        let raw_bytes = extract_bytes(&data_vec);
                    
                        let mut time_bytes = [0; 8];
                        time_bytes.copy_from_slice(&raw_bytes[..8.min(raw_bytes.len())]);
                    
                        /* for mut i in 0..8 {
                            time_bytes[i] = read_buffer[i];
                        } */
    
                        //get the timestamp
                        let time_int = i64::from_ne_bytes(time_bytes);
                        let time = DateTime::from_timestamp(time_int, 0).unwrap();
                        let time_string = format!("{}", time.format("%m/%d/%y %H:%M"));
                        
                        println!("time stamp: {:#?} (UTC)", time_string);
                    }
                
                    "message" => {
                        list_helper(&listarg2, &mut data_vec);
                        let raw_bytes = extract_bytes(&data_vec);
                    
                        //get the message string
                        let mut data_string = match str::from_utf8(&raw_bytes) {
                            Ok(data) => data,
                            Err(e) => "<data not a valid string>"
                        }.to_string();
                        data_string.retain(|c| c != '\0');
                        
                        println!("message string: {:#?}", data_string);
                    }
            
                    _ => {
                        println!("unrecognized command")
                    }
                }
            }
        }

        _ => println!("write command returned value: {success:#?}")
    }

}



fn list_helper(listdata: &str, data_vec: &mut Vec<GenericData>) {
    let child_scheme = libredox::call::open("/scheme/gtrand", O_RDONLY, 0).expect("could not open child/service base scheme");
    let read_buf: &mut [u8] = &mut [b'0'; 1024];
    let data_scheme = libredox::call::dup(child_scheme, listdata.as_bytes()).expect("could not dup fd");
    let size = libredox::call::read(data_scheme, read_buf).expect("could not read data scheme");
    data_vec.extend(read_buf[..size].iter().map(|&b| GenericData::Byte(b)));
}

fn extract_bytes(data_vec: &Vec<GenericData>) -> Vec<u8> {
    data_vec.iter()
        .filter_map(|d| if let GenericData::Byte(b) = d { Some(*b) } else { None })
        .collect()
}
