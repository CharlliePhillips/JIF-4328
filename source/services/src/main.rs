use clap::Parser;
use shared::{SMCommand, CommandResponse, TOMLMessage, get_response, format_uptime};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
};
use chrono::{self, Local, TimeZone};
use comfy_table;

#[derive(Parser)]
#[command(version, about, long_about = None, disable_help_subcommand = true)]
struct Cli {
    #[command(subcommand)]
    cmd: SMCommand,
}

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

    File::write(sm_fd, &cmd_bytes).expect("Failed to write command to service monitor");

    // print_response(&cli.cmd, sm_fd);
    let response_buf: Vec<u8> = get_response(sm_fd);
    if response_buf.len() > 0 {
        let s = std::str::from_utf8(&response_buf)
            .expect("Error parsing response to UTF8")
            .to_string();
        let response: CommandResponse = toml::from_str(&s).expect("Error parsing UTF8 to CommandResponse");
        match &response.message {
            Some(TOMLMessage::String(str)) => {
                println!("{str}");
            }
            Some(TOMLMessage::ServiceStats(stats)) => {
                let header_names = vec!["Name", "PID", "Uptime", "Message", "Status"];

                let mut table_fmt = comfy_table::Table::new();
                let mut headers = Vec::<comfy_table::Cell>::new();
                let mut rows: Vec<Vec<String>> = Vec::new();
                for h in header_names {
                    headers.push(comfy_table::Cell::new(&h).add_attribute(comfy_table::Attribute::Reverse));
                }
                for k in stats {
                    let mut row: Vec<String> = Vec::new();
                    row.push(k.name.clone());
                    row.push(if k.running {k.pid.to_string()} else {String::from("None")});
                    row.push(if k.running {format_uptime(k.time_init, k.time_now)} else {String::from("None")});
                    row.push(if k.running {k.message.clone()} else {String::from("None")});
                    row.push(if k.running {String::from("Running")} else {String::from("Not running")});
                    rows.push(row);
                }

                table_fmt.load_preset(comfy_table::presets::NOTHING)
                    .set_content_arrangement(comfy_table::ContentArrangement::Dynamic)
                    .set_header(headers)
                    .add_rows(rows)
                    ;

                println!("{table_fmt}");
            }
            _ => {
                if response.status.success {
                    println!("Command '{}' succeeded", response.status.command);
                }
                else {
                    println!("Command '{}' failed", response.status.command);
                }
            }
        }
    }
}
