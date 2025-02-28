use clap::Subcommand;

/// Command enum used by the services command line
#[derive(Subcommand)]
pub enum SMCommand {
    Start{
        service_name: String,
    },
    Stop{
        service_name: String,
    },
    List
}

impl SMCommand {
    /// Returns the lowercase name of the command as a String
    pub fn name(&self) -> String {
        match self {
            SMCommand::Stop{..} => {
                String::from("stop")
            },
            SMCommand::Start{..} => {
                String::from("start")
            },
            SMCommand::List => {
                String::from("list")
            },
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
        }
        command_string.into_bytes()
    }

    /// Attempts to convert a byte buffer back into an SMCommand
    pub fn from_bytes(buffer: &[u8]) -> Result<SMCommand, String> {
        let cmd_string: String = match String::from_utf8(buffer.to_vec()) {
            Ok(value) => value,
            Err(_) => return Err(String::from("No valid SMCommand name found in byte buffer"))
        };

        let cmd_tokens: Vec<&str> = cmd_string.split(" ").collect();
        if cmd_tokens.len() < 1 {
            return Err(String::from("No valid SMCommand name found in byte buffer"))
        }

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
            _ => {
                return Err(String::from("No valid SMCommand name found in byte buffer"))
            }
        }
    }
}