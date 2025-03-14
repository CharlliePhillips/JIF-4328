use clap::Parser;
use shared::SMCommand;
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
};

#[derive(Parser)]
#[command(version, about, long_about = None, disable_help_subcommand = true)]
struct Cli {
    #[command(subcommand)]
    cmd: SMCommand,
}

// todo: report back if service-name is invalid in start, stop commands
fn main() {
    let cli = Cli::parse();

    let Ok(sm_fd) = &mut OpenOptions::new()
        .write(true)
        .open("/scheme/service-monitor")
    else {
        panic!()
    };

    let cmd_bytes = &cli
        .cmd
        .encode()
        .expect("Failed to encode command to byte buffer");

    File::write(sm_fd, &cmd_bytes)
            .expect("Failed to write command to service monitor");

    // print_response(&cli.cmd, sm_fd);
    let response: Vec<u8> = get_response(sm_fd);
    if response.len() > 0 {
        // TODO: replace with TOML parsing and dynamically construct the correct printout
        println!("{}", std::str::from_utf8(&response)
            .expect("Error parsing response to UTF8")
            .to_string()
        );
    }
}

fn get_response(sm_fd: &mut File) -> Vec<u8> {
    let mut response = Vec::<u8>::new();
    loop {
        let mut buf = [0u8; 1024];
        let size =
            File::read(sm_fd, &mut buf).expect("Failed to read PIDs from service monitor");
        if size == 0 {
            break;
        }
        response.extend_from_slice(&buf[..size]);
    }
    return response;
}
