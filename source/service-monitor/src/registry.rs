use serde::Deserialize;
use std::{fs::File, io::Read, path::Path};

#[derive(Debug, Deserialize)]
pub struct Service {
    name: String,
    r#type: String,
    args: Vec<String>,
    manual_override: bool,
    depends: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct Registry {
    service: Vec<Service>,
}

fn read_registry() -> Registry {
    // TODO: determine filepath (where will registry.toml be located?)
    // ! This filepath is just a temporary solution
    let path: &Path = Path::new("registry.toml");
    let mut file = match File::open(&path) {
        Err(err) => panic!("Unable to open registry.toml: {}", err),
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

    let registry: Registry = toml::from_str(&toml_str).expect("Unable to parse registry.toml");
    return registry;
}
