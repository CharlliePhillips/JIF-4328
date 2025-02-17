use libredox::{call::{open, read, write}, flag::*};
use log::{error, info, warn, LevelFilter};
use redox_log::{OutputBuilder, RedoxLogger};
use redox_scheme::{Request, RequestKind, Scheme, SchemeBlock, SignalBehavior, Socket};
use std::{str, borrow::BorrowMut, fmt::{format, Debug}, fs::{File, OpenOptions}, io::{Read, Write}, os::{fd::AsRawFd, unix::fs::OpenOptionsExt}, process::{Child, Command, Stdio}};
use hashbrown::HashMap;
use scheme::SMScheme;
use timer;
use chrono::prelude::*;
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
    let mut services: HashMap<String, ServiceEntry> = read_registry();

    // start dependencies
    for service in services.values_mut() {
        let name: &str = service.name.as_str();
        service.time_started = Local::now().timestamp(); // where should this go?
        let mut child_service: Child = std::process::Command::new(name).spawn().expect("failed to start child service");
        child_service.wait();
        service.running = true;
        
        
        // SCRUM-39 TODO: this block should be turned into a new function that preforms this in a single here but can also
        // handle variable requests, maybe definining an enum with all the request types instead of a string would be helpful?

        // open the service's BaseScheme
        let child_scheme = libredox::call::open(service.scheme_path.clone(), O_RDWR, 0).expect("failed to open chld scheme");
        // dup into the pid scheme in order to read that data
        if let Ok(pid_scheme) = libredox::call::dup(child_scheme, b"pid") {
            // now we can read the pid onto the buffer from it's subscheme
            let mut read_buffer: &mut [u8] = &mut [b'0'; 32];
            libredox::call::read(pid_scheme, read_buffer).expect("could not read pid");
            // process the buffer based on the request (pid)
            let mut pid_bytes: [u8; 8] = [0; 8];
            for mut i in 0..7 {
                info!("byte {} reads {}", i, read_buffer[i]);
                pid_bytes[i] = read_buffer[i];
                i += 1;
            }
            // this last line could instead be something like let pid = getSvcAttr(service, "pid")
            let pid = usize::from_ne_bytes(pid_bytes);

            service.pid = pid;
            info!("started {} with pid: {:#?}", name, pid);
        } else {
            panic!("could not open pid scheme!");
        }
    }



    redox_daemon::Daemon::new(move |daemon| {
        let name = "service-monitor";
        let socket = Socket::create(name).expect("service-monitor: failed to create Service Monitor scheme");

        let mut sm_scheme = SMScheme{
            cmd: 0,
            arg1: String::from(""),
            pid_buffer: Vec::new(), //used in list, could be better as the BTreeMap later?
            info_buffer: Vec::new(),
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
                    let response = request.handle_scheme(&mut sm_scheme);
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

/// Checks if the service-monitor's command value has been changed and performs the appropriate action.
/// Currently supports the following commands:
/// - stop: check if service is running, if it is then get pid and stop
/// - start: check if service is running, if not build command from registry and start
/// - list: get all pids from managed services and return them to CLI
fn eval_cmd(services: &mut HashMap<String, ServiceEntry>, sm_scheme: &mut SMScheme) {
    const CMD_STOP: u32 = 1;
    const CMD_START: u32 = 2;
    const CMD_LIST: u32 = 3;
    const CMD_INFO: u32 = 4;

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
                            service.time_started = Local::now().timestamp(); // where should this go for the start command?
                            child.wait();
                            let child_scheme = libredox::call::open(service.scheme_path.clone(), O_RDWR, 1)
                                .expect("couldn't open child scheme");
                            let pid_req = b"pid";
                            let pid_scheme = libredox::call::dup(child_scheme, pid_req).expect("could not get pid");
                            
                            let read_buffer: &mut [u8] = &mut [b'0'; 32];
                            libredox::call::read(pid_scheme, read_buffer).expect("could not read pid");
                            // process the buffer based on the request
                            let mut pid_bytes: [u8; 8] = [0; 8];
                            for mut i in 0..8 {
                                //info!("byte {} reads {}", i, read_buffer[i]);
                                pid_bytes[i] = read_buffer[i];
                                i += 1;
                            }
                            let pid = usize::from_ne_bytes(pid_bytes);
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
        CMD_INFO => {
            // needs to pass the information to the buffers:
            // name
            // uptime
            // readcount
            // writecount
            // schemesize
            // errorcount
            // message
            // All in that order!
            // info will be gathered from the service, compiled into the formatted string, then encoded into the buffer as bytes, for services/main.rs to read from the scheme
            if let Some(service) = services.get_mut(&sm_scheme.arg1) {
                if service.running {
                    info!("found service: {}, grabbing info now", service.name);

                    let child_scheme = libredox::call::open(service.scheme_path.clone(), O_RDWR, 1).expect("couldn't open child scheme");
                    let read_buffer: &mut [u8] = &mut [b'0'; 32];

                    let req = b"request_count";
                    let time = b"time_stamp";
                    let message = b"message";


                    let message_scheme = libredox::call::dup(child_scheme, message).expect("could not dup message fd");
                    libredox::call::read(message_scheme, read_buffer);
                    // grab the string
                    let mut message_string = match str::from_utf8(&read_buffer){
                        Ok(data) => data,
                        Err(e) => "<data not a valid string>"
                    }.to_string();
                    // change trailing 0 chars into empty string
                    message_string.retain(|c| c != '\0');
                    //info!("~sm found a data string: {:#?}", message_string);

                    // get and print r/w tuple assume if there is a comma char at index 8 of the read
                    // bytes then assume bytes 0-7 = tuple.0 and 9-16 are tuple.1
                    let reqs_scheme = libredox::call::dup(child_scheme, req).expect("could not dup reqs fd");
                    libredox::call::read(reqs_scheme, read_buffer);
                    let mut read_int: i64 = 0;
                    let mut write_int: i64 = 0;
                    if read_buffer[8] == b',' {
                        let mut first_int_bytes = [0; 8];
                        let mut second_int_bytes = [0; 8];
                        for mut i in 0..8 {
                            first_int_bytes[i] = read_buffer[i];
                            second_int_bytes[i] = read_buffer[i + 9];
                        }
                        read_int = i64::from_ne_bytes(first_int_bytes);
                        write_int = i64::from_ne_bytes(second_int_bytes);
                        //info!("~sm read requests: {:#?}", read_int);
                        //info!("~sm write requests: {:#?}", write_int);
                    }

                    let time_scheme = libredox::call::dup(child_scheme, time).expect("could not dup time fd");
                    // set up the read buffer and read from the scheme into it
                    libredox::call::read(time_scheme, read_buffer).expect("could not read time response");
                    // process the buffer based on the request
                    let mut time_bytes = [0; 8];
                    for mut i in 0..8 {
                        time_bytes[i] = read_buffer[i];
                    }

                    // get the start time
                    let time_init_int = i64::from_ne_bytes(time_bytes);
                    let time_init = Local.timestamp_opt(time_init_int, 0).unwrap();
                    // get the current time
                    let current_time = Local::now();
                    // get the duration between the two
                    let duration = current_time.signed_duration_since(time_init);
                    let hours = duration.num_hours();
                    let minutes = duration.num_minutes() % 60;
                    let seconds = duration.num_seconds() % 60;
                    let millisecs = duration.num_milliseconds() % 1000;
                    let seconds_with_millis = format!("{:.3}", seconds as f64 + (millisecs as f64 / 1000.0));
                    let uptime_string = format!("{} hours, {} minutes, {} seconds", hours, minutes, seconds_with_millis);

                    info!("~sm time started registered versus time initialized: {}, {}", service.time_started, time_init_int);
                    let time_started = Local.timestamp_opt(service.time_started, 0).unwrap();
                    let init_duration = time_init.signed_duration_since(time_started);
                    let init_minutes = init_duration.num_minutes();
                    let init_seconds = init_duration.num_seconds() % 60;
                    let init_millisecs = init_duration.num_milliseconds() % 1000;
                    let init_seconds_with_millis = format!("{:.3}", init_seconds as f64 + (init_millisecs as f64 / 1000.0));
                    let time_init_string = format!("{} minutes, {} seconds", init_minutes, init_seconds_with_millis);


                    let mut info_string = format!(
                    "\nService: {} \nUptime: {} \nLast time to initialize: {} \nRead count: {} \nWrite count: {} \nScheme size: {} \nError count: {} \nMessage: \"{}\" ", 
                    service.name, uptime_string, time_init_string, read_int, write_int, 0, 0, message_string);
                    //info!("~sm info string: {:#?}", info_string);

                    // set the info buffer to the formatted info string
                    sm_scheme.info_buffer = info_string.as_bytes().to_vec();

                    // close the schemes
                    libredox::call::close(time_scheme);
                    libredox::call::close(reqs_scheme);
                    libredox::call::close(message_scheme);
                    libredox::call::close(child_scheme);

                } else {
                    // it should not fail to provide info, so this will need to be changed later
                    warn!("info failed: {} is not running", service.name);
                    sm_scheme.cmd = 0;
                    sm_scheme.arg1 = "".to_string();
                }
            } else {
                warn!("info failed: no service named '{}'", sm_scheme.arg1);
                sm_scheme.cmd = 0;
                sm_scheme.arg1 = "".to_string();
            }
        },
        _ => {}
    }
}


fn test_service_data(service: &mut ServiceEntry) {
    warn!("testing service data!");
    let child_scheme = libredox::call::open(service.scheme_path.clone(), O_RDWR, 0).expect("could not open child/service base scheme");
    let read_buffer: &mut [u8] = &mut [b'0'; 32];
    
    libredox::call::read(child_scheme, read_buffer).expect("could not read from child's main scheme");
    let mut rand_bytes = [0; 8];
    for mut i in 0..8 {
        rand_bytes[i] = read_buffer[i];
    }
    
    // get and print a random integer showing we can still read from gtrand's main scheme
    let rand_int = i64::from_ne_bytes(rand_bytes);
    info!("Read a random integer: {:#?}", rand_int);

    // set the request that we want and write it to the scheme
    let req = b"request_count";
    let time = b"time_stamp";
    let message = b"message";

    let time_scheme = libredox::call::dup(child_scheme, time).expect("could not dup time fd");
    // set up the read buffer and read from the scheme into it
    libredox::call::read(time_scheme, read_buffer).expect("could not read time response");
    // process the buffer based on the request
    let mut time_bytes = [0; 8];
    for mut i in 0..8 {
        time_bytes[i] = read_buffer[i];
    }
    
    // get and print the timestamp
    let time_int = i64::from_ne_bytes(time_bytes);
    let time = DateTime::from_timestamp(time_int, 0).unwrap();
    let time_string = format!("{}", time.format("%m/%d/%y %H:%M"));
    info!("time stamp: {:#?} (UTC)", time_string);

    // get and print r/w tuple assume if there is a comma char at index 8 of the read
    // bytes then assume bytes 0-7 = tuple.0 and 9-16 are tuple.1
    let reqs_scheme = libredox::call::dup(child_scheme, req).expect("could not dup reqs fd");
    libredox::call::read(reqs_scheme, read_buffer);
    if read_buffer[8] == b',' {
        let mut first_int_bytes = [0; 8];
        let mut second_int_bytes = [0; 8];
        for mut i in 0..8 {
            first_int_bytes[i] = read_buffer[i];
            second_int_bytes[i] = read_buffer[i + 9];
        }
        let first_int = i64::from_ne_bytes(first_int_bytes);
        let second_int = i64::from_ne_bytes(second_int_bytes);
        info!("read requests: {:#?}", first_int);
        info!("write requests: {:#?}", second_int);
    }

    let message_scheme = libredox::call::dup(child_scheme, message).expect("could not dup message fd");
    libredox::call::read(message_scheme, read_buffer);
    let mut data_string = match str::from_utf8(&read_buffer){
        Ok(data) => data,
        Err(e) => "<data not a valid string>"
    }.to_string();
    // change trailing 0 chars into empty string
    data_string.retain(|c| c != '\0');
    info!("data string: {:#?}", data_string);

    // lets mess around and test one of the other main scheme methods
    // if neither panics it can be assumed the main scheme (child_scheme) got both

    // this works
    let Ok(child_size) = libredox::call::fstat(child_scheme) else {panic!()};
    // this does not, the main scheme checks the id
    //let Ok(time_size) = libredox::call::fstat(time_scheme) else {panic!()};
    
    libredox::call::close(time_scheme);
    libredox::call::close(reqs_scheme);
    libredox::call::close(message_scheme);
    libredox::call::close(child_scheme);
}
