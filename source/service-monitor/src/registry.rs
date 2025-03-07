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
    pub read_count: i64,
    pub write_count: i64,
    pub error_count: i64,
    pub last_response_time: i64,
    pub message: String,
    pub total_reads: u64, 
    pub total_writes: u64,
    pub total_opens: u64,
    pub total_closes: u64,
    pub total_dups: u64,
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
            error_count: 0,
            last_response_time: 0,
            message: String::new(),
            total_reads: 0, 
            total_writes: 0,
            total_opens: 0,
            total_closes: 0,
            total_dups: 0,
        };
    services.insert(s.name, new_entry);
    }
    return services;
}

pub fn write_registry(registry : HashMap<String, ServiceEntry>) {
    // ! This filepath is just a temporary solution
    let path: &Path = Path::new("/usr/share/smregistry.toml");
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


// daemon is managed (new style), unmanaged is old-style

pub fn view_entry(name: &str) -> String {
    let services = read_registry();
    if let Some(entry) = services.get(name) {
        // these are just print statments for now, we'd want these to be in the CLI so they'd need to be passed back
        // but that won't occur until after refactoring
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
    r#type: &str, //if this string were to be -o, we'd write "unmanaged" instead of "daemon"
    args: &Vec<String>,
    scheme_path: &str,
    depends: &Vec<String>) 
{
    let path: &Path = Path::new("/usr/share/smregistry.toml");
    let mut file = OpenOptions::new()
        .write(true)
        .append(true)
        .open(&path)
        .expect("Unable to open smregistry.toml");

    let manual_override = false;
    let mut type_str = "daemon";
    if r#type == "-o" {
        // old style
        type_str = "unmanaged";
    }
    let new_entry_str = format!(
        "\n\n[[service]]\nname = \"{}\"\ntype = \"{}\"\nargs = {:?}\nmanual_override = {}\ndepends = {:?}\nscheme_path = \"{}\"\n",
        name, type_str, args, manual_override, depends, scheme_path
    );
    file.write_all(new_entry_str.as_bytes()).expect("Unable to write to smregistry.toml");
}

pub fn rm_entry(name: &str) { //later on once view returns a buffer, rm could use that to find the entry
    let mut services = read_registry();
    if let Some(entry) = services.get(name) {
        services.remove(name);
        write_registry(services);
    } else {
        println!("Service not found in registry");
    }
}
//add old, add new, rm, view