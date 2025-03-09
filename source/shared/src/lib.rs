use clap::{Subcommand};
use serde::{Deserialize, Serialize};
use regex::Regex;

/// Command enum used by the services command line
#[derive(Subcommand)]
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

#[derive(Subcommand)]
pub enum RegistryCommand {
    Add {
        #[arg(long)]
        old: bool,
        
        service_name: String, //required

        // we don't need r#type, we can just use the old boolean or default to "daemon".
        // #[arg(default_value = "daemon")]
        // r#type: String,
        
        #[arg(value_name = "start_args", help = "Arguments for starting the daemon", value_parser = validate_args)]
        args: Option<::std::vec::Vec<String>>, //mandatory

        #[arg(long = "override")]
        manual_override: bool, //this will default to false, if --override, it will be true 
        
        #[arg(value_name = "depends", help = "a list of dependencies for the daemon", value_parser = validate_args)]
        depends: Option<::std::vec::Vec<String>>, //mandatory, should default to empty vec?
        
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
    },
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

    /// Converts the command and its arguments into a byte vector for external use
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut command_string: String = self.name();
        match self {
            SMCommand::Stop{service_name} => {
                command_string.push(' ');
                command_string.push_str(&service_name);
            },
            SMCommand::Start{service_name} => {
                command_string.push(' ');
                command_string.push_str(&service_name);
            },
            SMCommand::List => {},
            SMCommand::Clear{service_name} => {
                command_string.push(' ');
                command_string.push_str(&service_name);
            },
            SMCommand::Info{service_name} => {
                command_string.push(' ');
                command_string.push_str(&service_name);
            },
            SMCommand::Registry{ subcommand } => {
                command_string.push_str(" ");
                match subcommand {
                    RegistryCommand::Add { service_name, old, args, manual_override, depends, scheme_path } => {

                        command_string.push_str(subcommand.name().as_str());
                        
                        command_string.push_str(if *old {" 1 "} else {" 0 "});

                        command_string.push_str(&service_name);

                        command_string.push_str(if *manual_override {" 1 "} else {" 0 "});

                        command_string.push_str(scheme_path);

                        command_string.push_str(" ");

                        
                        let args_str = format!("{:?}", args.as_deref().unwrap_or(&vec![]));
                        println!("ARGS");
                        println!("{:?}", args_str);
                        command_string.push_str(&args_str);

                        command_string.push_str(" ");

                        let depends_str = format!("{:?}", depends.as_deref().unwrap_or(&vec![]));
                        println!("DEPENDS");
                        println!("{:?}", depends_str);
                        command_string.push_str(&depends_str);

                    },
                    RegistryCommand::Remove { service_name } => {
                        command_string.push_str(subcommand.name().as_str());
                        command_string.push_str(" ");
                        command_string.push_str(service_name);
                    },
                    RegistryCommand::View { service_name } => {
                        command_string.push_str(subcommand.name().as_str());
                        command_string.push_str(" ");
                        command_string.push_str(service_name);
                    },
                    RegistryCommand::Edit { service_name } => {
                        command_string.push_str(subcommand.name().as_str());
                        command_string.push_str(" ");
                        command_string.push_str(service_name);
                    },
                }
            }
        }
        command_string.into_bytes()
    }

    /// Attempts to convert a byte buffer back into an SMCommand
    pub fn from_bytes(buffer: &[u8]) -> Result<SMCommand, String> {
        let cmd_string: String = match String::from_utf8(buffer.to_vec()) {
            Ok(value) => value,
            Err(_) => return Err(String::from("No valid SMCommand name found in byte buffer"))
        };
        //print!("{:?}", cmd_string);
        let cmd_tokens: Vec<&str> = cmd_string.split(" ").collect();
        // let mut remaining = None;
        // if cmd_tokens.len() > 5{
        //     remaining = Some(cmd_tokens[5..].join(" ")); //this makes no sense to me, but it only works if I declare this here instead of the if block for add.
        // }
        
        if cmd_tokens.len() < 1 {
            return Err(String::from("No valid SMCommand name found in byte buffer"))
        }

        // if cmd_tokens.len() > 1 && cmd_tokens[1] == "add" {
        //     //registry is 0, add is 1, old is 2, service_name is 3, manual_override is 4, scheme_path is 5, args is 6, depends is 7
        //     //special case break for registry add since args may have spaces, so we fix this in the same idea as cmd_tokens
        //     //example: "registry add 0 args=['arg0', 'arg1'] 0 depends=['dep0', 'dep1'] /path/to/scheme service_name"
            
        //     let args_regex = Regex::new(r"(args=\[[^]]*\])").unwrap();
        //     let depends_regex = Regex::new(r"(depends=\[[^]]*\])").unwrap();
            
        //     print!("{:?}", remaining);
        //     let remaining_str = remaining.as_deref().unwrap_or("");
        //     cmd_tokens[6] = args_regex.find(remaining_str).map(|m| m.as_str()).unwrap_or("");
        //     cmd_tokens[7] = depends_regex.find(remaining_str).map(|s| s.as_str()).unwrap_or("");
        // }

        match cmd_tokens[0] {
            "stop" => {
                if cmd_tokens.len() != 2 {
                    return Err(String::from("Invalid arguments for SMCommand 'stop'"))
                }
                return Ok(SMCommand::Stop { service_name: String::from(cmd_tokens[1]) });
            }
            "start" => {
                if cmd_tokens.len() != 2 {
                    return Err(String::from("Invalid arguments for SMCommand 'start'"))
                }
                return Ok(SMCommand::Start { service_name: String::from(cmd_tokens[1]) });
            }
            "list" => {
                return Ok(SMCommand::List);
            }
            "clear" => {
                if cmd_tokens.len() != 2 {
                    return Err(String::from("Invalid arguments for SMCommand 'clear'"))
                }
                return Ok(SMCommand::Clear { service_name: String::from(cmd_tokens[1]) });
            }
            "info" => {
                if cmd_tokens.len() != 2 {
                    return Err(String::from("Invalid arguments for SMCommand 'info'"))
                }
                return Ok(SMCommand::Info { service_name: String::from(cmd_tokens[1]) });
            }
            "registry" => {
                //TODO: define the Err return for this
                if cmd_tokens.len() < 2 {
                    return Err(String::from("Invalid arguments for SMCommand 'registry'"));
                }
                match cmd_tokens[1] {
                    "add" => {
                        // if cmd_tokens.len() != 8 {
                        //     return Err(String::from("Invalid arguments for SMCommand 'registry add'"));
                        // }
                        for token in &cmd_tokens {
                            println!("{}", token);
                        }
                        let old: bool = match cmd_tokens[2] {
                            "0" => false,
                            "1" => true,
                            _ => return Err(String::from("Invalid arguments for SMCommand 'registry add'"))
                        };
                        let manual_override: bool = match cmd_tokens[4] {
                            "0" => false,
                            "1" => true,
                            _ => return Err(String::from("Invalid arguments for SMCommand 'registry add'"))
                        };
                        let args: Vec<String> = validate_args(cmd_tokens[6]).unwrap();
                        let depends: Vec<String> = validate_args(cmd_tokens[7]).unwrap();

                        return Ok(SMCommand::Registry {
                            subcommand: RegistryCommand::Add {
                                old,
                                service_name: String::from(cmd_tokens[3]),
                                args: Some(args),
                                manual_override,
                                depends: Some(depends),
                                scheme_path: String::from(cmd_tokens[5]),
                            }
                        });
                    }
                    "remove" => {
                        if cmd_tokens.len() != 3 {
                            return Err(String::from("Invalid arguments for SMCommand 'registry remove'"));
                        }
                        return Ok(SMCommand::Registry {
                            subcommand: RegistryCommand::Remove {
                                service_name: String::from(cmd_tokens[2])
                            }
                        });
                    }
                    "view" => {
                        if cmd_tokens.len() != 3 {
                            return Err(String::from("Invalid arguments for SMCommand 'registry view'"));
                        }
                        return Ok(SMCommand::Registry {
                            subcommand: RegistryCommand::View {
                                service_name: String::from(cmd_tokens[2])
                            }
                        });
                    }
                    "edit" => {
                        if cmd_tokens.len() != 3 {
                            return Err(String::from("Invalid arguments for SMCommand 'registry edit'"));
                        }
                        return Ok(SMCommand::Registry {
                            subcommand: RegistryCommand::Edit {
                                service_name: String::from(cmd_tokens[2])
                            }
                        });
                    }
                    _ => {
                        return Err(String::from("Invalid arguments for SMCommand 'registry'"));
                    }
                }
            }
            _ => {
                return Err(String::from("No valid SMCommand name found in byte buffer"));
            }
        }
    }
}