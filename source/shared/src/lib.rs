use clap::Subcommand;
use std::{fs::File, io::Read, str};
use serde::{Deserialize, Serialize};
use chrono::{self, Local, TimeZone};

/// Command enum used by the services command line
#[derive(Subcommand, Serialize, Deserialize)]
#[serde(tag = "command")]
pub enum SMCommand {
    #[command(about = "Start a service")]
    Start {
        #[arg(help = "The name of the service")]
        service_name: String,
    },
    #[command(about = "Stop a service")]
    Stop {
        #[arg(help = "The name of the service")]
        service_name: String,
    },
    #[command(about = "List all services and their respective statuses")]
    List,
    #[command(about = "Clear short-term stats for a service")]
    Clear {
        #[arg(help = "The name of the service")]
        service_name: String,
    },
    #[command(about = "Get info about a service")]
    Info {
        #[arg(help = "The name of the service")]
        service_name: String,
    },
    #[command(about = "Change and view the registry. Try 'services registry --help' for more information")]
    Registry {
        #[command(subcommand)]
        subcommand: RegistryCommand,
    }
}

#[derive(Subcommand, Serialize, Deserialize)]
pub enum RegistryCommand {
    #[command(about = "Add a service to the registry")]
    Add {
        #[arg(long, help = "If present, indicates that the service is an old-style daemon")]
        old: bool,
        
        #[arg(help = "The name of the service")]
        service_name: String,
        
        #[arg(value_name = "start_args", help = "Arguments for starting the daemon", value_parser = validate_args)]
        args: Option<::std::vec::Vec<String>>,

        #[arg(long = "override", help = "If present, the service monitor will not override the fields in the registry")]
        manual_override: bool, //this will default to false, if --override, it will be true 
        
        #[arg(value_name = "depends", help = "A list of dependencies for the daemon", value_parser = validate_deps)]
        depends: Option<::std::vec::Vec<String>>,
        
        #[arg(help = "The path to the scheme file")]
        scheme_path: String,
    },
    #[command(about = "Remove a service's entry from the registry")]
    Remove {
        #[arg(help = "The name of the service")]
        service_name: String,
    },
    #[command(about = "Print a service's entry in the registry")]
    View {
        #[arg(help = "The name of the service")]
        service_name: String,
    },
    #[command(about = "Edit a service's entry in the registry")]
    Edit {
        #[arg(long, help = "If present, indicates that the service is an old-style daemon")]
        old: bool,

        #[arg(help = "The name of the service")]
        service_name: String,
        
        #[arg(value_name = "edit_args", help = "Arguments for starting the daemon", value_parser = validate_args)]
        edit_args: Option<::std::vec::Vec<String>>,
        
        #[arg(value_name = "depends", help = "A list of dependencies for the daemon", value_parser = validate_deps)]
        depends: Option<::std::vec::Vec<String>>,

        #[arg(help = "The path to the scheme file")]
        scheme_path: String,
        
        
    }
}

/// Validation function used to ensure the correct format is used for the `args` vector
fn validate_args(s: &str) -> Result<Vec<String>, String> {
    let mut parsed: String = String::from(s);
    if !parsed.starts_with("args=") {
        parsed.insert_str(0, "args=");
    }

    #[derive(Serialize, Deserialize)]
    struct Args {
        args: Vec<String>,
    }

    let vec: Args = match toml::from_str(&parsed) {
        Ok(v) => v,
        Err(e) => {
            return Err(format!("{}\n  Expected format: ['arg0', 'arg1', ... ]", e))
        },
    };

    return Ok(vec.args);    
}

/// Validation function used to ensure the correct format is used for the `deps` vector
fn validate_deps(s: &str) -> Result<Vec<String>, String> {
    let mut parsed: String = String::from(s);
    if !parsed.starts_with("deps=") {
        parsed.insert_str(0, "deps=");
    }

    #[derive(Serialize, Deserialize)]
    struct Deps {
        deps: Vec<String>,
    }

    let vec: Deps = match toml::from_str(&parsed) {
        Ok(v) => v,
        Err(e) => {
            return Err(format!("{}\n  Expected format: ['dep0', 'dep1', ... ]", e))
        },
    };

    return Ok(vec.deps);    
}

impl SMCommand {
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        toml::to_string(self)
            .map(|s| { s.into_bytes() })
            .map_err(|e| format!("Failed to encode SMCommand into string: {}", e))
    }

    pub fn decode(bytes: &[u8]) -> Result<SMCommand, String> {
        let toml_str = match str::from_utf8(bytes) {
            Ok(s) => s,
            Err(e) => return Err(format!("Failed to decode bytes into string: {}", e))
        };

        toml::from_str(toml_str)
            .map_err(|e| format!("Failed to decode bytes into SMCommand: {}", e))           
    }
}

#[derive(Serialize, Deserialize)]
pub struct ServiceRuntimeStats {
    pub name: String,
    pub pid: usize,
    pub time_init: i64,
    pub time_started: i64,
    pub time_now: i64,
    pub message: String,
    pub running: bool,

}

/// Message variant
#[derive(Serialize, Deserialize)]
pub enum TOMLMessage {
    String(String),
    ServiceStats(Vec<ServiceRuntimeStats>),
}

pub fn get_response(sm_fd: &mut File) -> Vec<u8> {
    let mut response = Vec::<u8>::new();
    loop {
        let mut buf = [0u8; 1024];
        let size = File::read(sm_fd, &mut buf).expect("Failed to read PIDs from service monitor");
        if size == 0 {
            break;
        }
        response.extend_from_slice(&buf[..size]);
    }
    return response;
}
    
// function that takes a time difference and returns a string of the time in hours, minutes, and seconds
pub fn format_uptime(start_time_ms: i64, end_time_ms: i64) -> String {
    let start = Local.timestamp_millis_opt(start_time_ms).unwrap();
    let end = Local.timestamp_millis_opt(end_time_ms).unwrap();
    
    let duration = end.signed_duration_since(start);
    
    let hours = duration.num_hours();
    let minutes = duration.num_minutes() % 60;
    let seconds = duration.num_seconds() % 60;
    let millisecs = duration.num_milliseconds() % 1000;
    let seconds_with_millis = format!("{:02}.{:03}", seconds, millisecs);
    
    let mut parts = Vec::new();
    
    if hours > 0 {
        parts.push(format!("{} hours", hours));
    }
    if minutes > 0 {
        parts.push(format!("{} minutes", minutes));
    }
    parts.push(format!("{} seconds", seconds_with_millis));
    
    parts.join(", ")
}