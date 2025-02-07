use libredox::{call::{open, read, write}, flag::{O_PATH, O_RDONLY}};
use log::{error, info, warn, LevelFilter};
use redox_log::{OutputBuilder, RedoxLogger};
use redox_scheme::{Request, RequestKind, Scheme, SchemeBlock, SchemeBlockMut, SchemeMut, SignalBehavior, Socket, V2};
use std::{str, borrow::BorrowMut, collections::BTreeMap, fmt::{format, Debug}, fs::{File, OpenOptions}, io::{Read, Write}, os::{fd::AsRawFd, unix::fs::OpenOptionsExt}, process::{Child, Command, Stdio}};
use scheme::{SMScheme};
use timer;
use chrono::{prelude::*};
use std::sync::mpsc::channel;
mod scheme;
mod registry;
use registry::{read_registry, ServiceEntry};

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
    let mut services: BTreeMap<String, ServiceEntry> = read_registry();

    // start dependencies
    for service in services.values_mut() {
        let name: &str = service.name.as_str();
        let mut child_service: Child = std::process::Command::new(name).spawn().expect("failed to start child service");
        child_service.wait();
        service.running = true;
        
        // SCRUM-39 TODO: this block should be turned into a new function that preforms this in a single here but can also
        // handle variable requests, maybe definining an enum with all the request types instead of a string would be helpful?

        // open the scheme to get 'child_scheme' fd
        let Ok(child_scheme) = &mut OpenOptions::new().write(true)
        .open(service.scheme_path.clone()) else {panic!()};
        // set the request that we want and write it to the scheme
        let pid_req = b"pid";
        File::write(child_scheme, pid_req).expect("could not request pid");
        // set up the read buffer and read from the scheme into it
        let mut read_buffer: &mut [u8] = &mut [b'0'; 32];
        File::read(child_scheme, read_buffer).expect("could not read pid");
        // process the buffer based on the request (pid)
        let mut pid_bytes: [u8; 8] = [0; 8];
        for mut i in 0..7 {
            info!("byte {} reads {}", i, read_buffer[i]);
            pid_bytes[i] = read_buffer[i];
            i += 1;
        }
        // this last line would instead be something like let pid = getSvcAttr(service, "pid")
        let pid = usize::from_ne_bytes(pid_bytes);

        service.pid = pid;
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
            

            eval_cmd(&mut services, &mut sm_scheme); 
            

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

fn test_service_data(service: &mut ServiceEntry) {
        warn!("testing service data!");
        let Ok(child_scheme) = &mut OpenOptions::new().write(true)
        .open(service.scheme_path.clone()) else {panic!()};
        // set the request that we want and write it to the scheme
        let test_req= b"request_count";

        File::write(child_scheme, test_req).expect("could not complete test request");
        // set up the read buffer and read from the scheme into it
        let read_buffer: &mut [u8] = &mut [b'0'; 32];
        File::read(child_scheme, read_buffer).expect("could not read test response");
        // process the buffer based on the request
        let mut test_bytes: [u8; 32] = [0; 32];
        for mut i in 0..32 {
            //info!("byte {} reads {}", i, read_buffer[i]);
            test_bytes[i] = read_buffer[i];
            i += 1;
        }

        info!("data bytes: {:#?}", test_bytes);
        let mut time_bytes = [0; 8];
        for mut i in 0..8 {
            time_bytes[i] = test_bytes[i];
        }
        
        // get and print the timestamp
        let time_int = i64::from_ne_bytes(time_bytes);
        let time = DateTime::from_timestamp(time_int, 0).unwrap();
        let time_string = format!("{}", time.format("%m/%d/%y %H:%M"));
        info!("time stamp: {:#?} (UTC)", time_string);

        // get and print r/w tuple assume if there is a comma char at index 8 of the read
        // bytes then assume bytes 0-7 = tuple.0 and 9-16 are tuple.1
        if test_bytes[8] == b',' {
            let mut second_int_bytes = [0; 8];
            for mut i in 9..17 {
                second_int_bytes[i - 9] = test_bytes[i];
            }
            let second_int = i64::from_ne_bytes(second_int_bytes);
            info!("read requests: {:#?}", time_int);
            info!("write requests: {:#?}", second_int)
        }
        let mut data_string = match str::from_utf8(&test_bytes){
            Ok(data) => data,
            Err(e) => "<data not a valid string>"
        }.to_string();
        // change trailing 0 chars into empty string
        data_string.retain(|c| c != '\0');
        info!("data string: {:#?}", data_string)
}
/// Checks if the service-monitor's command value has been changed and performs the appropriate action.
/// Currently supports the following commands:
/// - stop: check if service is running, if it is then get pid and stop
/// - start: check if service is running, if not build command from registry and start
/// - list: get all pids from managed services and return them to CLI
fn eval_cmd(services: &mut BTreeMap<String, ServiceEntry>, sm_scheme: &mut SMScheme) {
    const CMD_STOP: u32 = 1;
    const CMD_START: u32 = 2;
    const CMD_LIST: u32 = 3;

    match sm_scheme.cmd {
        CMD_STOP => {
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
        },
        CMD_START => {
            if let Some(service) = services.get_mut(&sm_scheme.arg1) {
                // can add args here later with '.arg()'
                if (!service.running) {
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
                    warn!("service: '{}' is already running", service.name);
                    test_service_data(service);
                }
            } else {
                warn!("start failed: no service named '{}'", sm_scheme.arg1);
            }
            //reset the current command value
            sm_scheme.cmd = 0;
            sm_scheme.arg1 = "".to_string();
        },
        CMD_LIST => {
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
        },
        _ => {}
    }
}
