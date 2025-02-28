use serde::{Deserialize, Serialize};
use chrono::prelude::*;
use std::{fs::File, fs::OpenOptions, io::Read, io::Write, path::Path};
use hashbrown::HashMap;
use log::{error, info, warn};


#[derive(Debug, Deserialize, Serialize)]
pub struct Service {
    name: String,
    r#type: String,
    args: Vec<String>,
    manual_override: bool,
    depends: Vec<String>,
    scheme_path: String,
}

// we may want to consider the visibility of these a little more carefully, all set to pub rn to make things work.
// this def needs a better name though.
pub struct ServiceEntry {
    pub name: String,
    pub r#type: String,
    pub args: Vec<String>,
    pub manual_override: bool,
    pub depends: Vec<String>,
    pub scheme_path: String,
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
        Ok(_) => {},
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
            name: s.name.clone(),
            r#type: s.r#type,
            args: s.args,
            manual_override: s.manual_override,
            depends: s.depends,
            scheme_path: s.scheme_path,
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

        };
    services.insert(s.name, new_entry);
    }
    return services;
}

pub fn write_registry(registry : HashMap<String, ServiceEntry>) {
    let path: &Path = Path::new("/usr/share/smregistry.toml"); //same as read_registry, this filepath is temporary.
    let mut file = match File::create(&path) {
        Err(err) => panic!("Unable to open smregistry.toml: {}", err),
        Ok(file) => file,
    };
    let vals = registry.values();
    let mut reconstructed : Vec<Service> = Vec::new();
    for val in vals {
        let new_service = Service {
            name: val.name.clone(),
            r#type: val.r#type.clone(),
            args: val.args.clone(),
            manual_override: val.manual_override,
            depends: val.depends.clone(),
            scheme_path: val.scheme_path.clone(),
        };
        reconstructed.push(new_service);
    }
    let registry_struct = Registry {
        service: reconstructed,
    };
    let mut toml_str: String = toml::to_string(&registry_struct).unwrap();
    match file.write_all(&mut toml_str.as_bytes()) {
        Err(err) => panic!("Unable to read registry.toml as string: {}", err),
        Ok(_) => {},
    };
}

pub fn view_entry(name: &str) -> String {
    let services = read_registry();
    if let Some(entry) = services.get(name) {
        let entry_string = format!(
            "Service Name: {} \nType: {} \nArgs: {:?} \nManual Override: {} \nDepends: {:?} \nScheme Path: {}",
            entry.name, entry.r#type, entry.args, entry.manual_override, entry.depends, entry.scheme_path
        );
        return entry_string;
    } else {
        return String::from("Service not found in registry");
    }
}

pub fn add_entry(
    name: &str,
    r#type: &str, //if --old, this is "unmanaged" instead of "daemon"
    args: &Vec<String>,
    manual_override: bool,
    scheme_path: &str,
    depends: &Vec<String>) 
{
    let mut services = read_registry();
    let new_entry = ServiceEntry {
        name: name.to_string(),
        r#type: r#type.to_string(),
        args: args.to_vec(),
        manual_override: manual_override,
        depends: depends.to_vec(),
        scheme_path: scheme_path.to_string(),
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

    };
    services.insert(name.to_string(), new_entry);
    write_registry(services);
    
}

pub fn rm_entry(name: &str) {
    let mut services = read_registry();
    if let Some(entry) = services.get(name) {
        services.remove(name);
        write_registry(services);
    } else {
        println!("Service not found in registry");
    }
}

pub fn edit_entry(name: &str, o: bool, edit_args: &Vec<String>, scheme_path: &str, dependencies: &Vec<String>) {
    let mut services = read_registry();
    if let Some(entry) = services.get_mut(name) {
        if entry.running {
            warn!("Service is currently running");
        }
        
        if o {
            entry.r#type = "unmanaged".to_string();
        }
            
        if !edit_args.is_empty() {
            entry.args = edit_args.clone();
        }
        
        if !scheme_path.is_empty() {
            entry.scheme_path = scheme_path.to_string();
        } else if entry.scheme_path.is_empty() {
            entry.scheme_path = format!("/scheme/{}", name);
        }
        
        for dep in dependencies {
            if !entry.depends.contains(dep) {
                entry.depends.push(dep.clone());
            }
        }
            
        write_registry(services);
    } else {
        println!("Service not found in registry\nRegistry edit failed");
    }
}

pub fn edit_hash_entry(
    services: &mut HashMap<String, ServiceEntry>, 
    name: & str,
    o: bool, 
    edit_args: &Vec<String>,
    scheme_path: &str,
    depends: &Vec<String>)
{
    if services.contains_key(name) {
        let mut entry = services.get_mut(name).unwrap();
        if o {
            entry.r#type = "unmanaged".to_string();
        }
        if !edit_args.is_empty() {
            entry.args = edit_args.to_vec();
        }
        
        if !scheme_path.is_empty() {
            entry.scheme_path = scheme_path.to_string();
        } else if entry.scheme_path.is_empty() {
            entry.scheme_path = format!("/scheme/{}", name);
        }
        for dep in depends {
            if !entry.depends.contains(dep) {
                entry.depends.push(dep.clone());
            }
        }
        
        let new_entry = ServiceEntry {
            name: entry.name.to_string(),
            r#type: entry.r#type.clone(),
            args: entry.args.clone(),
            manual_override: entry.manual_override,
            depends: entry.depends.clone(),
            scheme_path: entry.scheme_path.clone(),
            running: entry.running,
            pid: entry.pid,
            time_started: entry.time_started,
            time_init: entry.time_init,
            read_count: entry.read_count,
            write_count: entry.write_count,
            error_count: entry.error_count,
            last_response_time: entry.last_response_time,
            message: entry.message.clone(),
            total_reads: entry.total_reads, 
            total_writes: entry.total_writes,
            total_opens: entry.total_opens,
            total_closes: entry.total_closes,
            total_dups: entry.total_dups,
        };
        
        services.insert(name.to_string(), new_entry);
    } else {
        println!("Unable to edit Service Entry that is not present in internal list");
    }
}

pub fn rm_hash_entry(services: &mut HashMap<String, ServiceEntry>, name: & str) {
    let mut services_toml = read_registry();
    if let Some(entry) = services_toml.get(name) {
        println!("Service is still present in registry, unable to remove from internal list");
    } else {
        if services.contains_key(name) {    
            let mut entry = services.get(name).unwrap();
            if entry.running {
                println!("Cannot remove an entry that is currently running");
            } else {
                services.remove(name);
                println!("Removing service from internal list");
            }
        } else {
            println!("Cannot find entry in internal list to remove");
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
    services: &mut HashMap<String, ServiceEntry>
)
{
    
    if services.contains_key(name) {
        println!("Cannot add entry that is already present in internal list");
    } else {
        let new_entry = ServiceEntry {
            name: name.to_string(),
            r#type: r#type.to_string(),
            args: args.to_vec(),
            manual_override: manual_override,
            depends: depends.to_vec(),
            scheme_path: scheme_path.to_string(),
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

        };
        services.insert(name.to_string(), new_entry);
    }
}
//add old, add new, rm, view
