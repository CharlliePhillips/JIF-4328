//! Crate containing structs and functions shared by `service-monitor` and its front-ends

use clap::Subcommand;
use std::{fs::File, io::Read, str};
use serde::{Deserialize, Serialize};
use chrono::{self, Local, TimeZone};

/// Command enum used by the services command line
#[derive(Subcommand, Serialize, Deserialize, Clone)]
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

impl std::fmt::Display for SMCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SMCommand::Start { service_name: _ } => write!(f, ""),
            SMCommand::Stop { service_name: _ } => write!(f, ""),
            SMCommand::List => write!(f, "list"),
            SMCommand::Clear { service_name: _ } => write!(f, "clear"),
            SMCommand::Info { service_name: _ } => write!(f, "info"),
            SMCommand::Registry { subcommand } => write!(f, "registry {}", subcommand),
        }
    }
}

impl std::fmt::Display for RegistryCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegistryCommand::Add { old: _, service_name: _, args: _, manual_override: _, depends: _, scheme_path: _ } => write!(f, "add"),
            RegistryCommand::Remove { service_name: _ } => write!(f, "remove"),
            RegistryCommand::View { service_name: _ } => write!(f, "view"),
            RegistryCommand::Edit { old: _, service_name: _, edit_args: _, depends: _, scheme_path: _ } => write!(f, "edit"),
        }
    }
}

/// Registry subcommand used by the services command line to view/edit the registry
#[derive(Subcommand, Serialize, Deserialize, Clone)]
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
    /// Converts this SMCommand into a TOML string stored in a byte buffer
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        toml::to_string(self)
            .map(|s| { s.into_bytes() })
            .map_err(|e| format!("Failed to encode SMCommand into string: {}", e))
    }

    /// Converts a byte buffer containing a TOML string into its original [SMCommand] if possible
    pub fn decode(bytes: &[u8]) -> Result<SMCommand, String> {
        let toml_str = match str::from_utf8(bytes) {
            Ok(s) => s,
            Err(e) => return Err(format!("Failed to decode bytes into string: {}", e))
        };

        toml::from_str(toml_str)
            .map_err(|e| format!("Failed to decode bytes into SMCommand: {}", e))           
    }
}

/// Struct defining the response generated after running an [SMCommand]
#[derive(Serialize, Deserialize)]
pub struct CommandResponse {
    /// Info regarding the command
    pub status: CommandStatus,
    /// Optional message the command may attach to its response
    pub message: Option<TOMLMessage>,
}

impl CommandResponse {
    /// Creates a new [CommandResponse] using the given command, success flag, and optional message.
    /// This function does not take ownership of `command` (it is cloned), but does take ownership of `message`.
    pub fn new(command: &SMCommand, success: bool, message: Option<TOMLMessage>) -> CommandResponse {
        CommandResponse{status: CommandStatus {command: command.clone(), success: success}, message: message}
    }
}

/// Struct containing info about the [SMCommand] that was run and whether it succeeded
#[derive(Serialize, Deserialize)]
pub struct CommandStatus {
    /// A copy of the command struct that was run
    pub command: SMCommand,
    /// True if command was successfully executed, false otherwise
    pub success: bool,
}

/// Struct containing data about a registered service's runtime stats.
/// This is used primarily for the `services list` command.
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

#[derive(Serialize, Deserialize)]
pub struct ServiceDetailStats {
    pub name: String,
    pub pid: usize,
    pub time_init: i64,
    pub time_started: i64,
    pub time_now: i64,
    pub read_count: u64,
    pub write_count: u64,
    pub open_count: u64,
    pub close_count: u64,
    pub dup_count: u64,
    pub error_count: u64,
    pub total_reads: u64,
    pub total_writes: u64,
    pub total_opens: u64,
    pub total_closes: u64,
    pub total_dups: u64,
    pub total_errors: u64,
    pub message: String,
    pub running: bool,
}


/// Enum defining types of messages we may expect to get from a [CommandResponse]
#[derive(Serialize, Deserialize)]
pub enum TOMLMessage {
    String(String),
    ServiceStats(Vec<ServiceRuntimeStats>),
    ServiceDetail(ServiceDetailStats),
}

/// Reads the command responsed buffer from the service-monitor's scheme.
/// # Panics
/// If reading the file fails, this may cause a panic.
// todo: Graceful error handling (put into a `Result<Vec<u8>, String>`, prevent timeouts?)
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
    
/// Function that takes a time difference and returns a string of the time in hours, minutes, and seconds
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