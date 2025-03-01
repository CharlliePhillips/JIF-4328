use libredox::{call::{open, read, write}, flag::*, error::*, errno::*};
use log::{error, info, warn, LevelFilter};
use redox_log::{OutputBuilder, RedoxLogger};
use redox_scheme::{Request, RequestKind, Scheme, SchemeBlock, SignalBehavior, Socket};
use shared::SMCommand;
use std::{str, borrow::BorrowMut, fmt::{format, Debug}, fs::{File, OpenOptions}, io::{Read, Write}, os::{fd::AsRawFd, unix::fs::OpenOptionsExt}, process::{Child, Command, Stdio}};
use hashbrown::HashMap;
use scheme::SMScheme;
use timer;
use chrono::prelude::*;
use std::sync::mpsc::channel;
mod scheme;
mod registry;
use registry::{read_registry, ServiceEntry};

enum GenericData {
    Byte(u8),
    Short(u16),
    Int(u32),
    Text(String)
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
    let mut services: HashMap<String, ServiceEntry> = read_registry();

    // start dependencies
    for service in services.values_mut() {
        let name: &str = service.name.as_str();
        service.time_started = Local::now().timestamp_millis(); // where should this go?
        let mut child_service: Child = std::process::Command::new(name).spawn().expect("failed to start child service");
        child_service.wait();
        service.running = true;
        
        
        // SCRUM-39 TODO: this block should be turned into a new function that preforms this in a single here but can also
        // handle variable requests, maybe defining an enum with all the request types instead of a string would be helpful?

        // open the service's BaseScheme
        let child_scheme = libredox::call::open(service.scheme_path.clone(), O_RDWR, 0).expect("failed to open child scheme");
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
            cmd: None,
            response_buffer: Vec::new(),
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
            // not sure if that is still how it works or not, but seems similar to this code
            // get request 
             
            let Some(request) = socket
                .next_request(SignalBehavior::Restart)
                .expect("service-monitor: failed to read events from Service Monitor scheme")
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

// todo: automatically reset sm_scheme.cmd without having to do it in every branch condition
// todo: figure out how to unify resets to sm_scheme.cmd (currently split btwn here and services/main.rs)
/// Checks if the service-monitor's command value has been changed and performs the appropriate action.
/// Currently supports the following commands:
/// - stop: check if service is running, if it is then get pid and stop
/// - start: check if service is running, if not build command from registry and start
/// - list: get all pids from managed services and return them to CLI
fn eval_cmd(services: &mut HashMap<String, ServiceEntry>, sm_scheme: &mut SMScheme) {
    match &sm_scheme.cmd {
        Some(SMCommand::Stop { service_name }) => {
            if let Some(service) = services.get_mut(service_name) {
                info!("Stopping '{}'", service.name);
                stop(service, sm_scheme);
            } else {
                warn!("stop failed: no service named '{}'", service_name);
            }
            // reset the current command value
            sm_scheme.cmd = None;
        },
        Some(SMCommand::Start { service_name }) => {
            if let Some(service) = services.get_mut(service_name) {
                info!("Starting '{}'", service.name);
                start(service, sm_scheme);
            } else {
                warn!("start failed: no service named '{}'", service_name);
            }
            // reset the current command value
            sm_scheme.cmd = None;
        },
        Some(SMCommand::List) => {
            list(services, sm_scheme)
            // ! do not reset the current command value -> wait for scheme.rs to handle it
        },
        Some(SMCommand::Clear { service_name }) => {
            if let Some(service) = services.get_mut(service_name) {
                info!("Clearing short-term stats for '{}'", service.name);
                clear(service);
            }
            // reset the current command value
            sm_scheme.cmd = None;
        },
        Some(SMCommand::Info { service_name }) => {
            if let Some(service) = services.get_mut(service_name) {
                info!("Finding information for '{}'", service.name);
                info(service, sm_scheme);
            } else {
                warn!("info failed: no service named '{}'", service_name);
                // reset the current command value
                sm_scheme.cmd = None;
            }
        },
        None => {},
        _ => {}
    }
}

fn update_service_info(service: &mut ServiceEntry) {
    info!("Updating information for: {}", service.name);

    let child_scheme = libredox::call::open(service.scheme_path.clone(), O_RDWR, 1).expect("couldn't open child scheme");
    let read_buffer: &mut [u8] = &mut [b'0'; 48];

    let req = "request_count";
    let time = "time_stamp";
    let message = "message";


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
    service.message = message_string;

    // get and print read, write, open, close, & dup count, they are successive u64 bytes read from requests subscheme 
    let reqs_scheme = libredox::call::dup(child_scheme, req).expect("could not dup reqs fd");
    libredox::call::read(reqs_scheme, read_buffer);
    
    let mut read_bytes: [u8; 8] = [0; 8];
    let mut write_bytes: [u8; 8] = [0; 8];
    let mut open_bytes: [u8; 8] = [0; 8];
    let mut close_bytes: [u8; 8] = [0; 8];
    let mut dup_bytes: [u8; 8] = [0; 8];
    let mut error_bytes: [u8; 8] = [0; 8];
    read_bytes.clone_from_slice(&read_buffer[0..8]);
    write_bytes.clone_from_slice(&read_buffer[8..16]);
    open_bytes.clone_from_slice(&read_buffer[16..24]);
    close_bytes.clone_from_slice(&read_buffer[24..32]);
    dup_bytes.clone_from_slice(&read_buffer[32..40]);
    error_bytes.clone_from_slice(&read_buffer[40..48]);
    service.read_count = u64::from_ne_bytes(read_bytes);
    service.write_count = u64::from_ne_bytes(write_bytes);
    service.open_count = u64::from_ne_bytes(open_bytes);
    service.close_count = u64::from_ne_bytes(close_bytes);
    service.dup_count = u64::from_ne_bytes(dup_bytes);
    service.error_count = u64::from_ne_bytes(error_bytes);


    // get and process the message
    rHelper(service, read_buffer, message);
    let mut message_string = match str::from_utf8(&read_buffer){
        Ok(data) => data,
        Err(e) => "<data not a valid string>"
    }.to_string();
    // change trailing 0 chars into empty string
    message_string.retain(|c| c != '\0');
    //info!("~sm found a data string: {:#?}", message_string);
    service.message = message_string;

    // get and process the start time
    rHelper(service, read_buffer, time);
    let mut time_bytes = [0; 8];
    for mut i in 0..8 {
        time_bytes[i] = read_buffer[i];
    }
    let time_init_int = i64::from_ne_bytes(time_bytes);
    service.time_init = time_init_int;
}





fn stop(service: &mut ServiceEntry, sm_scheme: &mut SMScheme) {
    if service.running {
        info!("trying to kill pid {}", service.pid);
        let killRet = syscall::call::kill(service.pid, syscall::SIGKILL);
        service.running = false;
    } else {
        warn!("stop failed: {} was already stopped", service.name);
    }
}



fn start(service: &mut ServiceEntry, sm_scheme: &mut SMScheme) {
    // can add args here later with '.arg()'
    if (!service.running) {
        match std::process::Command::new(service.name.as_str()).spawn() {
            Ok(mut child) => {
                //service.pid = child.id().try_into().unwrap();
                //service.pid += 2;
                service.time_started = Local::now().timestamp_millis(); // where should this go for the start command?
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
        //test_service_data(service);
        if (service.name == "gtrand2") {
            test_timeout(service);
        }
        // When we actually report the total number of reads/writes, it should actually be the total added
        // to whatever the current value in the service is, the toal stored in the service monitor is
        // updated when the service's count is cleared.
        info!("total reads: {}, total writes: {}", service.total_reads, service.total_writes);
    }
}





fn info(service: &mut ServiceEntry, sm_scheme: &mut SMScheme) {
    if service.running {
        update_service_info(service);

        // set up time strings
        let uptime_string = time_string(service.time_init, Local::now().timestamp_millis());
        let time_init_string = time_string(service.time_started, service.time_init);
        info!("~sm time started registered versus time initialized: {}, {}", service.time_started, service.time_init);

        // set up the info string
        let mut info_string = format!(
        "\nService: {} \nUptime: {} \nLast time to initialize: {} \nRead count: {} \nWrite count: {} \nError count: {} \nMessage: \"{}\" ", 
        service.name, uptime_string, time_init_string, service.read_count, service.write_count, service.error_count, service.message);
        //info!("~sm info string: {:#?}", info_string);

        // set the info buffer to the formatted info string
        sm_scheme.response_buffer = info_string.as_bytes().to_vec();

    } else {
        // it should not fail to provide info, so this will need to be changed later
        warn!("info failed: {} is not running", service.name);
        sm_scheme.cmd = None;
    }
}

fn list(service_map: &mut HashMap<String, ServiceEntry>, sm_scheme: &mut SMScheme) {
    let mut endString:String = "Name | PID | Uptime | Message | Status\n".to_string();
    
    //let mut listString = "";
    //hashmap_bytes(services, sm_scheme);
    for service in service_map.values_mut() {
        //let service = services.get_mut(&sm_scheme.arg1)
        if service.running {
            update_service_info(service);
            // set up time strings
            let uptime_string = time_string(service.time_init, Local::now().timestamp_millis());
            
            let listString = format!("{} | {} | {} | {} | Running\n", service.name, service.pid, uptime_string, service.message);
            info!("line: {}", listString);
            endString.push_str(&listString);
            
            info!("End: {}", endString);
            info!("{:#?}", sm_scheme.response_buffer.as_ptr());
        } else {
            let listString = format!("{} | none | none | none | not running\n", service.name);
        }
    }
        
    sm_scheme.response_buffer = endString.as_bytes().to_vec();

}

// function that takes a time difference and returns a string of the time in hours, minutes, and seconds
fn time_string(start_time: i64, end_time: i64) -> String {
    let start = Local.timestamp_millis_opt(start_time).unwrap();
    let end = Local.timestamp_millis_opt(end_time).unwrap();
    
    let duration = end.signed_duration_since(start);

    let hours = duration.num_hours();
    let minutes = duration.num_minutes() % 60;
    let seconds = duration.num_seconds() % 60;
    let millisecs = duration.num_milliseconds() % 1000;
    let seconds_with_millis = format!("{:02}.{:03}", seconds, millisecs);

    let mut parts = Vec::new();

    if hours > 0 {
        parts.push(format!("{} hours", hours));
    }
    if minutes > 0 {
        parts.push(format!("{} minutes", minutes));
    }
    parts.push(format!("{} seconds", seconds_with_millis));

    parts.join(", ")
}




fn clear(service: &mut ServiceEntry) {
    // open the service scheme
    let child_scheme = libredox::call::open(service.scheme_path.clone(), O_RDWR, 0)
                .expect("couldn't open child scheme");
    // open the management subschemes
    let cntl_scheme = libredox::call::dup(child_scheme, b"control").expect("could not get cntl");
    let reqs_scheme = libredox::call::dup(child_scheme, b"request_count").expect("couldn't get request_count");
    
    // read the requests into a buffer
    let read_buffer: &mut [u8] = &mut [b'0'; 48];
    libredox::call::read(reqs_scheme, read_buffer);

    // turn that buffer into read/write as integers
    let mut read_bytes: [u8; 8] = [0; 8];
    let mut write_bytes: [u8; 8] = [0; 8];
    let mut open_bytes: [u8; 8] = [0; 8];
    let mut close_bytes: [u8; 8] = [0; 8];
    let mut dup_bytes: [u8; 8] = [0; 8];
    let mut error_bytes: [u8; 8] = [0; 8];
    read_bytes.clone_from_slice(&read_buffer[0..8]);
    write_bytes.clone_from_slice(&read_buffer[8..16]);
    open_bytes.clone_from_slice(&read_buffer[16..24]);
    close_bytes.clone_from_slice(&read_buffer[24..32]);
    dup_bytes.clone_from_slice(&read_buffer[32..40]);
    error_bytes.clone_from_slice(&read_buffer[40..48]);
    // count this for our service's totals
    service.total_reads += u64::from_ne_bytes(read_bytes);
    service.total_writes += u64::from_ne_bytes(write_bytes);
    service.total_opens += u64::from_ne_bytes(open_bytes);
    service.total_closes += u64::from_ne_bytes(close_bytes);
    service.total_dups += u64::from_ne_bytes(dup_bytes);
    service.total_errors += u64::from_ne_bytes(error_bytes);

    // clear the data and close the schemes.            
    libredox::call::write(cntl_scheme, b"clear").expect("could not write to cntl");
    libredox::call::close(cntl_scheme).expect("failed to close cntl");
    libredox::call::close(child_scheme).expect("failed to close child");
}

fn test_timeout(gtrand2: &mut ServiceEntry) {
    let timeout_req =  "timeout";
    wHelper(gtrand2, "", timeout_req);
    let read_buf = &mut [b'0';32];

    // for now we expect this to hang, 
    rHelper(gtrand2, read_buf,"");
    // future success? message
    info!("gtrand 2 timed out!");
}

fn test_count_ops(service: &mut ServiceEntry) -> i64 {
    let read_buf = &mut [b'0';8];
    rHelper(service, read_buf, "");
    info!("successfully read random {}", i64::from_ne_bytes(*read_buf));
    wHelper(service, "", "");
    return i64::from_ne_bytes(*read_buf);
}

fn test_err(gtrand2: &mut ServiceEntry) {
    let timeout_req =  "error";
    wHelper(gtrand2, "", timeout_req);
    let read_buf = &mut [b'0';32];
    // for now we expect this to hang, 
    match rHelper(gtrand2, read_buf,"") {
        Ok(i) => {
            // whatever happens here, do nothing, just testing
            warn!("test error failed!");
        }
        Err(e) => {
            // whatever happens here, do nothing, just testing
            info!("test error success!");
        }
    }
}

fn rHelper(service: &mut ServiceEntry, read_buf: &mut [u8], data: &str) -> Result<usize>{
    match libredox::call::open(service.scheme_path.clone(), O_RDWR, 0) {
        Ok(child_scheme) => {
            if !data.is_empty() {
                let data_scheme = libredox::call::dup(child_scheme, data.as_bytes())?;
                libredox::call::close(child_scheme);
                let result = libredox::call::read(data_scheme, read_buf);
                return result;
            } else {
                let result = libredox::call::read(child_scheme, read_buf);
                libredox::call::close(child_scheme);
                return result;
            }
        }
        // if we failed to open the base scheme
        _ => {
            return Err(Error::new(EBADF));
        }
    }
}

fn wHelper(service: &mut ServiceEntry, subscheme_name: &str, data: &str) {
    let child_scheme = libredox::call::open(service.scheme_path.clone(), O_RDWR, 0).expect("could not open child/service base scheme");
    let subscheme = libredox::call::dup(child_scheme, subscheme_name.as_bytes()).expect("could not dup fd");
    libredox::call::write(subscheme, data.as_bytes()).expect("could not write to scheme");
    libredox::call::close(subscheme);
}

fn extract_bytes(data_vec: &Vec<GenericData>) -> Vec<u8> {
    data_vec.iter()
        .filter_map(|d| if let GenericData::Byte(b) = d { Some(*b) } else { None })
        .collect()
}
