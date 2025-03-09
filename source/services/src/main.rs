use std::{borrow::BorrowMut, fmt::{format, Debug}, fs::{File, OpenOptions}, io::{Read, Write}, os::{fd::AsRawFd, unix::fs::OpenOptionsExt}, process::{Command, Stdio}};
use libredox::{call::{open, read, write}, flag::{O_PATH, O_RDONLY}};
use clap::{Parser, Subcommand, Args};    
use shared::{SMCommand, RegistryCommand};


#[derive(Parser)]
#[command(version, about, long_about = None, disable_help_subcommand = true)]
struct Cli {
    #[command(subcommand)]
    cmd: SMCommand,
}

// todo: replace with Clap crate for robust parsing and help-text
// todo: report back if service-name is invalid in start, stop commands
fn main() {
    let cli = Cli::parse();

    let Ok(sm_fd) = &mut OpenOptions::new().write(true)
    .open("/scheme/service-monitor") else {panic!()};

    let cmd_bytes = &cli.cmd.encode().expect("Failed to encode command to byte buffer");
    let success = File::write(sm_fd, &cmd_bytes).expect("Failed to write command to service monitor");

    if success == 0 {
        print_response(&cli.cmd, sm_fd);
    }
}

fn print_response(cmd: &SMCommand, sm_fd: &mut File) {
    match cmd {
        SMCommand::List | SMCommand::Info { service_name: _ } => {
            get_response_message(sm_fd);
        }
        SMCommand::Registry { subcommand } => {
            match subcommand {
                RegistryCommand::View { service_name: _ } => {
                    get_response_message(sm_fd);
                }
                _ => {}
                
            }
        }
        _ => {}
    }
}

fn get_response_message(sm_fd: &mut File) {
    let mut response_buffer = vec![0u8; 1024]; // 1024 is kinda arbitrary here, may cause issues later
    let size = File::read(sm_fd, &mut response_buffer).expect("Failed to read PIDs from service monitor");
    response_buffer.truncate(size);
    
    let mut data_string = match std::str::from_utf8(&response_buffer){
        Ok(data) => data,
        Err(e) => "<data not a valid string>"
    }.to_string();
    data_string.retain(|c| c != '\0');

    println!("{}", data_string);
}