use clap::Subcommand;
use serde::{Deserialize, Serialize};
use bincode::{Decode, Encode};

/// Command enum used by the services command line
#[derive(Subcommand, Encode, Decode)]
pub enum SMCommand {
    Start {
        service_name: String,
    },
    Stop {
        service_name: String,
    },
    List,
    Clear {
        service_name: String,
    },
    Info {
        service_name: String,
    },
    Registry {
        #[command(subcommand)]
        subcommand: RegistryCommand,
    }
}

#[derive(Subcommand, Encode, Decode)]
pub enum RegistryCommand {
    Add {
        #[arg(long)]
        old: bool,
        
        service_name: String, //required

        // we don't need r#type, we can just use the old flag or default to "daemon".
        
        #[arg(value_name = "start_args", help = "Arguments for starting the daemon", value_parser = validate_args)]
        args: Option<::std::vec::Vec<String>>, //mandatory

        #[arg(long = "override", help = "if not present, the service monitor may override the fields in the registry")]
        manual_override: bool, //this will default to false, if --override, it will be true 
        
        #[arg(value_name = "depends", help = "a list of dependencies for the daemon", value_parser = validate_args)]
        depends: Option<::std::vec::Vec<String>>, //mandatory
        
        scheme_path: String, //mandatory
    },
    Remove {
        service_name: String,
    },
    View {
        service_name: String,
    },
    Edit {
        service_name: String,
        
        #[arg(long = "o", help = "-o for old-style daemon")]
        o: bool,
        
        #[arg(value_name = "edit_args", help = "Arguments for the daemon", value_parser = validate_args)]
        edit_args: Option<::std::vec::Vec<String>>,
        
        scheme_path: String,
        
        #[arg(value_name = "dependencies", help = "A list of dependencies for the daemon", value_parser = validate_args)]
        dependencies: Option<::std::vec::Vec<String>>,
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
