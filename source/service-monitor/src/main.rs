use libredox::{call::{open, read, write}, flag::{O_PATH, O_RDONLY}};
use log::{error, info, warn, LevelFilter};
use redox_log::{OutputBuilder, RedoxLogger};
use redox_scheme::{Request, RequestKind, Scheme, SchemeBlock, SchemeBlockMut, SchemeMut, SignalBehavior, Socket, V2};
use std::{borrow::BorrowMut, collections::BTreeMap, fmt::{format, Debug}, fs::{File, OpenOptions}, io::{Read, Write}, os::{fd::AsRawFd, unix::fs::OpenOptionsExt}, process::{Child, Command, Stdio}};
use scheme::{SMScheme};
use timer;
use chrono;
use std::sync::mpsc::channel;
mod scheme;

struct ServiceEntry {
    name: String,
    running: bool,
    pid: usize,
    scheme_path: String
}

fn main() {
    let _ = RedoxLogger::new()
    .with_output(
        OutputBuilder::stdout()
            .with_filter(log::LevelFilter::Debug)
            .with_ansi_escape_codes()
            .build()
    )
    .with_process_name("service-monitor".into())
    .enable();
    info!("service-monitor logger started");
    
    // make list of managed services
    let mut services: BTreeMap<String, ServiceEntry> = BTreeMap::new();

    let name: String = String::from("service-monitor_gtrand");
    let gtrand_entry = ServiceEntry {
        name: name.clone(),
        running: false,
        pid: 0,
        scheme_path: String::from("/scheme/gtrand")
    };
    services.insert(name, gtrand_entry);

    // start dependencies
    for service in services.values_mut() {
        let name: &str = service.name.as_str();
        let mut child_service: Child = std::process::Command::new(name).spawn().expect("failed to start child service");
        child_service.wait();
        service.running = true;
        
        // daemonization process makes this id not the actual one we need
        // but it is two most of the time?
        let Ok(child_scheme) = &mut OpenOptions::new().write(true)
        .open(service.scheme_path.clone()) else {panic!()};
        let pid_req = b"pid";
        let pid: usize = File::write(child_scheme, pid_req).expect("could not get pid");
        //pid += 2;
        service.pid = pid;
        // TODO once pid can be read from scheme
        // fd = open(/scheme/<name>)
        // buf = "pid"
        // pid = write(fd, "pid")
        info!("started {} with pid: {:#?}", name, pid);
    }



    redox_daemon::Daemon::new(move |daemon| {
        let name = "service-monitor";
        let socket = Socket::<V2>::create(name).expect("service-monitor: failed to create Service Monitor scheme");

        let mut sm_scheme = SMScheme{
            cmd: 0,
            arg1: String::from(""),
            pid_buffer: Vec::new(), //used in list, could be better as the BTreeMap later?
        };
        
        info!("service-monitor daemonized with pid: {}", std::process::id());
        daemon.ready().expect("service-monitor: failed to notify parent");
        loop {
            /*
            TODO parse registry for updates, this could be skipped while running if no request to edit the registry is pending

             if a new entry is found then add it to the services vector in the SM scheme
             if there is only one entry then check if it is the placeholder and change it
             if it's the last service being removed then replace with placeholder

             once the services vector is updated use the information to start the list
            */
            
            // check if the service-monitor's command value has been changed.
            // stop: check if service is running, if it is then get pid and stop
            if sm_scheme.cmd == 1 {
                if let Some(service) = services.get_mut(&sm_scheme.arg1) {
                    if service.running {
                        info!("trying to kill pid {}", service.pid);
                        let killRet = syscall::call::kill(service.pid, syscall::SIGKILL);
                        service.running = false;
                    } else {
                        warn!("stop failed: {} was already stopped", service.name);
                    }
                } else {
                    warn!("stop failed: no service named '{}'", sm_scheme.arg1);
                }
                //reset the current command value
                sm_scheme.cmd = 0;
                sm_scheme.arg1 = "".to_string();
            }
            // start: check if service is running, if not build command from registry and start
            if sm_scheme.cmd == 2  {
                if let Some(service) = services.get_mut(&sm_scheme.arg1) {
                    // can add args here later with '.arg()'
                    match std::process::Command::new(service.name.as_str()).spawn() {
                        Ok(mut child) => {
                            //service.pid = child.id().try_into().unwrap();
                            //service.pid += 2;
                            child.wait();
                            let Ok(child_scheme) = &mut OpenOptions::new().write(true)
                            .open(service.scheme_path.clone()) else {panic!()};
                            let pid_req = b"pid";
                            let pid: usize = File::write(child_scheme, pid_req).expect("could not get pid");
                            service.pid = pid;
                            info!("child started with pid: {:#?}", service.pid);
                            service.running = true;
                        },

                        Err(e) => {
                            warn!("start failed: could not start {}", service.name);
                        }
                    };
                } else {
                    warn!("start failed: no service named '{}'", sm_scheme.arg1);
                }
                //reset the current command value
                sm_scheme.cmd = 0;
                sm_scheme.arg1 = "".to_string();
            }

            // list: get all pids from managed services and return them to CLI
            if sm_scheme.cmd == 3  {
                let mut pids: Vec<usize> = Vec::new();
                for service in services.values() {
                    if (service.running) {
                        pids.push(service.pid);
                    }
                }
                info!("Listing PIDs: {:?}", pids);
                let mut bytes: Vec<u8> = Vec::new();
                for pid in pids {
                    let pid_u32 = pid as u32;
                    bytes.extend_from_slice(&pid_u32.to_ne_bytes());
                }
                //info!("PIDs as bytes: {:?}", bytes);
                sm_scheme.pid_buffer = bytes;
            } 


            

            // The following is for handling requests to the SM scheme
            // Redox does timers with the timer scheme according to docs https://doc.redox-os.org/book/event-scheme.html
            // not sure if that is still how it works or not, but seems simmilar to this code
            // get request 
             
            let Some(request) = socket
                .next_request(SignalBehavior::Restart)
                .expect("service-monitor: failed to read events from Service Moniotr scheme")
            else {
                warn!("exiting Service Monitor");
                std::process::exit(0);
            };

            match request.kind() {
                RequestKind::Call(request) => {

                    // handle request
                    let response = request.handle_scheme_mut(&mut sm_scheme);
                    socket
                        .write_responses(&[response], SignalBehavior::Restart)
                        .expect("service-monitor: failed to write responses to Service Monitor scheme");

                }
                _ => (),
            }
        }
    })
    .expect("service-monitor: failed to daemonize");
}
