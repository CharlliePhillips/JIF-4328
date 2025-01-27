use serde::Deserialize;
use std::{fs::File, io::Read, path::Path, collections::BTreeMap};


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
}

#[derive(Debug, Deserialize)]
struct Registry {
    service: Vec<Service>,
}

pub fn read_registry() -> BTreeMap<String, ServiceEntry> {
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
    let mut services: BTreeMap<String, ServiceEntry> = BTreeMap::new();
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
        };
    services.insert(s.name, new_entry);
    }
    return services;
}
