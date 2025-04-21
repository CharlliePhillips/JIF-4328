use hashbrown::HashMap;
use log::warn;
use serde::{Deserialize, Serialize};
use shared::{TOMLMessage};
use std::{fs::File, io::Read, io::Write, path::Path};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Service {
    pub name: String,
    pub r#type: String,
    pub args: Vec<String>,
    pub manual_override: bool,
    pub depends: Vec<String>,
    pub scheme_path: String,
}

// we may want to consider the visibility of these a little more carefully, all set to pub rn to make things work.
// this def needs a better name though.
pub struct ServiceEntry {
    pub config: Service,
    pub running: bool,
    pub pid: usize,
    pub time_started: i64,
    pub time_init: i64,
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
    pub last_response_time: i64,
    pub message: String,
    pub message_time: i64,
    pub last_update_time: i64,
}

#[derive(Debug, Deserialize, Serialize)]
struct Registry {
    service: Vec<Service>,
}

pub fn read_registry() -> HashMap<String, ServiceEntry> {
    // TODO: determine filepath (where will registry.toml be located?)
    // ! This filepath is just a temporary solution
    let path: &Path = Path::new("/usr/share/smregistry.toml");
    let mut file = match File::open(&path) {
        Err(err) => panic!("Unable to open smregistry.toml: {}", err),
        Ok(file) => file,
    };

    let mut toml_str: String = String::new();
    match file.read_to_string(&mut toml_str) {
        Err(err) => panic!("Unable to read registry.toml as string: {}", err),
        Ok(_) => {}
    };

    // dev comment: can access data like so:
    // assert_eq!(registry.service.len(), 2);
    // assert_eq!(registry.service[0].name, "zerod");
    // assert_eq!(registry.service[0].r#type, "daemon");
    // assert_eq!(registry.service[0].manual_override, false);
    // assert!(registry.service[0].args.is_empty());
    // assert!(registry.service[0].depends.is_empty());

    // Sets up the services map for main.
    let mut services: HashMap<String, ServiceEntry> = HashMap::new();
    let registry: Registry = toml::from_str(&toml_str).expect("Unable to parse registry.toml");
    for s in registry.service {
        let new_entry = ServiceEntry {
            config: s,
            running: false,
            pid: 0,
            time_started: 0,
            time_init: 0,
            read_count: 0,
            write_count: 0,
            open_count: 0,
            close_count: 0,
            dup_count: 0,
            error_count: 0,
            total_reads: 0,
            total_writes: 0,
            total_opens: 0,
            total_closes: 0,
            total_dups: 0,
            total_errors: 0,
            last_response_time: 0,
            message: String::new(),
            message_time: 0,
            last_update_time: 0,
        };
        services.insert(new_entry.config.name.clone(), new_entry);
    }
    return services;
}

// todo: avoid panics: -> Result<Some<TOMLMessage>, Some<TOMLMessage>>
pub fn write_registry(registry: HashMap<String, ServiceEntry>) {
    let path: &Path = Path::new("/usr/share/smregistry.toml"); //same as read_registry, this filepath is temporary.
    let mut file = match File::create(&path) {
        Err(err) => panic!("Unable to open smregistry.toml: {}", err),
        Ok(file) => file,
    };
    let vals = registry.values();
    let mut reconstructed: Vec<Service> = Vec::new();
    for val in vals {
        let new_service = val.config.clone();
        reconstructed.push(new_service);
    }
    let registry_struct = Registry {
        service: reconstructed,
    };
    let toml_str: String = toml::to_string(&registry_struct).unwrap();
    match file.write_all(&mut toml_str.as_bytes()) {
        Err(err) => panic!("Unable to read registry.toml as string: {}", err),
        Ok(_) => {}
    };
}

pub fn view_entry(name: &str) -> Result<Option<TOMLMessage>, Option<TOMLMessage>> {
    let services = read_registry();
    if let Some(entry) = services.get(name) {
        let entry_string = format!(
            "Service Name: {} \nType: {} \nArgs: {:?} \nManual Override: {} \nDepends: {:?} \nScheme Path: {}",
            entry.config.name, entry.config.r#type, entry.config.args, entry.config.manual_override, entry.config.depends, entry.config.scheme_path
        );
        Ok(Some(TOMLMessage::String(entry_string)))
    } else {
        Err(Some(TOMLMessage::String(String::from("Service not found in registry"))))
    }
}

pub fn add_entry(
    name: &str,
    r#type: &str, //if --old, this is "unmanaged" instead of "daemon"
    args: &Vec<String>,
    manual_override: bool,
    scheme_path: &str,
    depends: &Vec<String>,
) -> Result<Option<TOMLMessage>, Option<TOMLMessage>> {
    let mut services = read_registry();
    let new_entry = ServiceEntry {
        config: Service {
            name: name.to_string(),
            r#type: r#type.to_string(),
            args: args.to_vec(),
            manual_override: manual_override,
            depends: depends.to_vec(),
            scheme_path: scheme_path.to_string(),
        },
        running: false,
        pid: 0,
        time_started: 0,
        time_init: 0,
        read_count: 0,
        write_count: 0,
        open_count: 0,
        close_count: 0,
        dup_count: 0,
        error_count: 0,
        total_reads: 0,
        total_writes: 0,
        total_opens: 0,
        total_closes: 0,
        total_dups: 0,
        total_errors: 0,
        last_response_time: 0,
        message: String::new(),
        message_time: 0,
        last_update_time: 0,
    };
    services.insert(name.to_string(), new_entry);
    write_registry(services);

    Ok(Some(TOMLMessage::String(format!("Successfully added service '{}' to registry", name))))
}

pub fn rm_entry(name: &str) -> Result<Option<TOMLMessage>, Option<TOMLMessage>> {
    let mut services = read_registry();
    if let Some(_entry) = services.get(name) {
        services.remove(name);
        write_registry(services);
        Ok(Some(TOMLMessage::String(format!("Successfully removed service '{}' from registry", name))))
    } else {
        //println!("Service not found in registry");
        Err(Some(TOMLMessage::String(format!("Unable to remove '{}' from registry: service not found", name))))
    }
}

pub fn edit_entry(
    name: &str,
    old: bool,
    edit_args: &Vec<String>,
    scheme_path: &str,
    depends: &Vec<String>,
) -> Result<Option<TOMLMessage>, Option<TOMLMessage>> {
    let mut services = read_registry();
    if let Some(entry) = services.get_mut(name) {
        if entry.running {
            warn!("Service is currently running");
        }

        if old {
            entry.config.r#type = "unmanaged".to_string();
        }

        if !edit_args.is_empty() {
            entry.config.args = edit_args.clone();
        }

        if !scheme_path.is_empty() {
            entry.config.scheme_path = scheme_path.to_string();
        } else if entry.config.scheme_path.is_empty() {
            entry.config.scheme_path = format!("/scheme/{}", name);
        }

        for dep in depends {
            if !entry.config.depends.contains(dep) {
                entry.config.depends.push(dep.clone());
            }
        }

        write_registry(services);
        Ok(Some(TOMLMessage::String(format!("Successfully edited service '{}' in registry", name))))
    } else {
        //println!("Service not found in registry\nRegistry edit failed");
        Err(Some(TOMLMessage::String(format!("Unable to edit '{}' in registry: service not found", name))))
    }
}

pub fn edit_hash_entry(
    services: &mut HashMap<String, ServiceEntry>,
    name: &str,
    old: bool,
    edit_args: &Vec<String>,
    scheme_path: &str,
    depends: &Vec<String>,
) -> Result<Option<TOMLMessage>, Option<TOMLMessage>> {
    if services.contains_key(name) {
        let entry = services.get_mut(name).unwrap();

        if old {
            entry.config.r#type = "unmanaged".to_string();
        }
        if !edit_args.is_empty() {
            entry.config.args = edit_args.to_vec();
        }

        if !scheme_path.is_empty() {
            entry.config.scheme_path = scheme_path.to_string();
        } else if entry.config.scheme_path.is_empty() {
            entry.config.scheme_path = format!("/scheme/{}", name);
        }
        for dep in depends {
            if !entry.config.depends.contains(dep) {
                entry.config.depends.push(dep.clone());
            }
        }

        // TODO: why are we cloning the entry we just edited?
        let new_entry = ServiceEntry {
            config: entry.config.clone(),
            running: entry.running,
            pid: entry.pid,
            time_started: entry.time_started,
            time_init: entry.time_init,
            read_count: entry.read_count,
            write_count: entry.write_count,
            error_count: entry.error_count,
            last_response_time: entry.last_response_time,
            message: entry.message.clone(),
            message_time: entry.message_time,
            total_reads: entry.total_reads,
            total_writes: entry.total_writes,
            total_opens: entry.total_opens,
            total_closes: entry.total_closes,
            total_dups: entry.total_dups,
            open_count: entry.open_count,
            close_count: entry.close_count,
            dup_count: entry.dup_count,
            total_errors: entry.total_errors,
            last_update_time: entry.last_update_time,
        };

        services.insert(name.to_string(), new_entry);

        // ! temp
        Ok(Some(TOMLMessage::String(format!("Successfully edited service '{}' in internal list", name))))
    } else {
        //println!("Unable to edit Service Entry that is not present in internal list");
        Err(Some(TOMLMessage::String(format!("Unable to edit '{}' in internal list: service not found", name))))
    }
}

pub fn rm_hash_entry(services: &mut HashMap<String, ServiceEntry>, name: &str) -> Result<Option<TOMLMessage>, Option<TOMLMessage>> {
    let services_toml = read_registry();
    if let Some(_entry) = services_toml.get(name) {
        Err(Some(TOMLMessage::String(format!("Unable to remove '{}' from internal list; service is still present in the registry", name))))
        //println!("Service is still present in registry, unable to remove from internal list");
    } else {
        if services.contains_key(name) {
            let entry = services.get(name).unwrap();
            if entry.running {
                // todo: msg: "Running service has been removed from the registry. It will be removed from the internal list when the service is stopped."
                Err(Some(TOMLMessage::String(format!("Unable to remove '{}' from internal list; service is still running", name))))
                //println!("Cannot remove an entry that is currently running");
            } else {
                services.remove(name);
                Ok(Some(TOMLMessage::String(format!("Removed '{}' from internal list", name))))
                //println!("Removing service from internal list");
            }
        } else {
            Err(Some(TOMLMessage::String(format!("Unable to remove '{}' from internal list; service not found", name))))
            //println!("Cannot find entry in internal list to remove");
        }
    }
}

pub fn add_hash_entry(
    name: &str,
    r#type: &str, //if this string were to be -o, we'd write "unmanaged" instead of "daemon"
    args: &Vec<String>,
    manual_override: bool,
    scheme_path: &str,
    depends: &Vec<String>,
    services: &mut HashMap<String, ServiceEntry>,
) -> Result<Option<TOMLMessage>, Option<TOMLMessage>> {
    if services.contains_key(name) {
        Err(Some(TOMLMessage::String(format!("Unable to add '{}' to internal list: service already present", name))))
        //println!("Cannot add entry that is already present in internal list");
    } else {
        let new_entry = ServiceEntry {
            config: Service {
                name: name.to_string(),
                r#type: r#type.to_string(),
                args: args.to_vec(),
                manual_override: manual_override,
                depends: depends.to_vec(),
                scheme_path: scheme_path.to_string(),
            },
            running: false,
            pid: 0,
            time_started: 0,
            time_init: 0,
            read_count: 0,
            write_count: 0,
            open_count: 0,
            close_count: 0,
            dup_count: 0,
            error_count: 0,
            total_reads: 0,
            total_writes: 0,
            total_opens: 0,
            total_closes: 0,
            total_dups: 0,
            total_errors: 0,
            last_response_time: 0,
            message: String::new(),
            message_time: 0,
            last_update_time: 0,
        };
        services.insert(name.to_string(), new_entry);
        Ok(Some(TOMLMessage::String(format!("Successfully added service '{}' to internal list", name))))
    }
}
//add old, add new, rm, view
