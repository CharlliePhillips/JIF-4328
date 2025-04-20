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
            Some(TOMLMessage::ServiceDetail(detail)) => {
                // todo: set up time strings
                let uptime_string = format_uptime(detail.time_init, Local::now().timestamp_millis());
                let time_init_string = format_uptime(detail.time_started, detail.time_init);

                let mut table_fmt1 = comfy_table::Table::new();
                let mut table_fmt2 = comfy_table::Table::new();
                
                let mut rows1: Vec<Vec<String>> = Vec::new();
                let mut rows2: Vec<Vec<String>> = Vec::new();

                let mut service_row: Vec<String> = Vec::new();
                let mut uptime_row: Vec<String> = Vec::new();
                let mut init_row: Vec<String> = Vec::new();
                let mut message_row: Vec<String> = Vec::new();
                let mut read_row: Vec<String> = Vec::new();
                let mut write_row: Vec<String> = Vec::new();
                let mut open_row: Vec<String> = Vec::new();
                let mut close_row: Vec<String> = Vec::new();
                let mut error_row: Vec<String> = Vec::new();
                if detail.running {
                    service_row.push("Service:".to_string());
                    service_row.push(detail.name.clone());
                    uptime_row.push("Uptime:".to_string());
                    uptime_row.push(format_uptime(detail.time_init, detail.time_now));
                    init_row.push("Time to init:".to_string());
                    init_row.push(format_uptime(detail.time_started, detail.time_init));
                    message_row.push("Message:".to_string());
                    message_row.push(detail.message.clone());
                    rows1.push(service_row);
                    rows1.push(uptime_row);
                    rows1.push(init_row);
                    rows1.push(message_row);
                
                    read_row.push("Live READ count:".to_string());
                    read_row.push(format!("{}", detail.read_count));
                    read_row.push("total:".to_string());
                    read_row.push(format!("{}", detail.total_reads));
                    write_row.push("Live WRITE count:".to_string());
                    write_row.push(format!("{}", detail.write_count));
                    write_row.push("total:".to_string());
                    write_row.push(format!("{}", detail.total_writes));
                    open_row.push("Live OPEN count:".to_string());
                    open_row.push(format!("{}", detail.open_count));
                    open_row.push("total:".to_string());
                    open_row.push(format!("{}", detail.total_opens));
                    close_row.push("Live CLOSE count:".to_string());
                    close_row.push(format!("{}", detail.close_count));
                    close_row.push("total:".to_string());
                    close_row.push(format!("{}", detail.total_closes));
                    error_row.push("Live ERROR count:".to_string());
                    error_row.push(format!("{}", detail.error_count));
                    error_row.push("total:".to_string());
                    error_row.push(format!("{}", detail.total_errors));
                    rows2.push(read_row);
                    rows2.push(write_row);
                    rows2.push(open_row);
                    rows2.push(close_row);
                    rows2.push(error_row);

                    table_fmt1.load_preset(comfy_table::presets::NOTHING)
                        .set_content_arrangement(comfy_table::ContentArrangement::Dynamic)
                        .add_rows(rows1)
                        ;

                    table_fmt2.load_preset(comfy_table::presets::NOTHING)
                        .set_content_arrangement(comfy_table::ContentArrangement::Dynamic)
                        .add_rows(rows2)
                        ;
                    println!("{table_fmt1}");
                    println!("{table_fmt2}");
                } else {
                    service_row.push("Service:".to_string());
                    service_row.push(detail.name.clone());
                    message_row.push("Message:".to_string());
                    message_row.push(detail.message.clone());
                    rows1.push(service_row);
                    rows1.push(message_row);

                
                    read_row.push("Total READ count:".to_string());
                    read_row.push(format!("{}", detail.total_reads));
                    write_row.push("Total WRITE count:".to_string());
                    write_row.push(format!("{}", detail.total_writes));
                    open_row.push("Total OPEN count:".to_string());
                    open_row.push(format!("{}", detail.total_opens));
                    close_row.push("Total CLOSE count:".to_string());
                    close_row.push(format!("{}", detail.total_closes));
                    error_row.push("Total ERROR count:".to_string());
                    error_row.push(format!("{}", detail.total_errors));
                    rows1.push(read_row);
                    rows1.push(write_row);
                    rows1.push(open_row);
                    rows1.push(close_row);
                    rows1.push(error_row);
                    table_fmt1.load_preset(comfy_table::presets::NOTHING)
                        .set_content_arrangement(comfy_table::ContentArrangement::Dynamic)
                        .add_rows(rows1)
                        ;

                    println!("{table_fmt1}");
                }

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
