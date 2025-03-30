use clap::Subcommand;
use serde::{Deserialize, Serialize};
use bincode::{Decode, Encode};

/// Command enum used by the services command line
#[derive(Subcommand, Encode, Decode)]
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

#[derive(Subcommand, Encode, Decode)]
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
        
        #[arg(value_name = "depends", help = "A list of dependencies for the daemon", value_parser = validate_args)]
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
        
        #[arg(value_name = "depends", help = "A list of dependencies for the daemon", value_parser = validate_args)]
        depends: Option<::std::vec::Vec<String>>,

        #[arg(help = "The path to the scheme file")]
        scheme_path: String,
        
        
    }
}

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


impl RegistryCommand {
    pub fn name(&self) -> String {
        match self {
            RegistryCommand::Add{..} => String::from("add"),
            RegistryCommand::Remove{..} => String::from("remove"),
            RegistryCommand::View{..} => String::from("view"),
            RegistryCommand::Edit{..} => String::from("edit"),
        }
    }
}

impl SMCommand {
    /// Returns the lowercase name of the command as a String
    pub fn name(&self) -> String {
        match self {
            SMCommand::Stop{..} => String::from("stop"),
            SMCommand::Start{..} => String::from("start"),
            SMCommand::List => String::from("list"),
            SMCommand::Info{..} => String::from("info"),
            SMCommand::Clear{..} => String::from("clear"),
            SMCommand::Registry{..} => String::from("registry"),
        }
    }

    pub fn encode(&self) -> Result<Vec<u8>, String> {
        bincode::encode_to_vec(self, bincode::config::standard()).map_err(|e| format!("Failed to encode SMCommand into bytes: {}", e))
    }

    pub fn decode(bytes: &[u8]) -> Result<SMCommand, String> {
        bincode::decode_from_slice(bytes, bincode::config::standard())
            .map(|c| c.0)
            .map_err(|e| format!("Failed to decode bytes into SMCommand: {}", e))
    }
}
