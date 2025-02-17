use serde::Deserialize;
use chrono::prelude::*;
use std::{fs::File, io::Read, path::Path};
use hashbrown::HashMap;


#[derive(Debug, Deserialize)]
pub struct Service {
    name: String,
    r#type: String,
    args: Vec<String>,
    manual_override: bool,
    depends: Vec<String>,
    scheme_path: String,
}

//we may want to consider the visibility of these a little more carefully, all set to pub rn to make things work.
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
}

#[derive(Debug, Deserialize)]
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
        };
    services.insert(s.name, new_entry);
    }
    return services;
}
