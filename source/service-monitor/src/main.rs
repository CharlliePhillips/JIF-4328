use chrono::prelude::*;
use hashbrown::HashMap;
use libredox::{
    errno::*,
    error::*,
    flag::*,
};
use log::{error, info, warn};
use redox_log::{OutputBuilder, RedoxLogger};
use redox_scheme::{RequestKind, SignalBehavior, Socket};
use scheme::SMScheme;
use shared::{CommandResponse, RegistryCommand, SMCommand, ServiceDetailStats, ServiceRuntimeStats, TOMLMessage};

use std::{
    str,
    sync::mpsc,
    thread,
    time::Duration,
};
mod registry;
mod scheme;
use registry::{
    add_entry, add_hash_entry, edit_entry, edit_hash_entry, read_registry, rm_entry, rm_hash_entry,
    view_entry, ServiceEntry,
};

fn main() {
    let _ = RedoxLogger::new()
        .with_output(
            OutputBuilder::stdout()
                .with_filter(log::LevelFilter::Debug)
                .with_ansi_escape_codes()
                .build(),
        )
        .with_process_name("service-monitor".into())
        .enable();
    info!("service-monitor logger started");

    redox_daemon::Daemon::new(move |daemon| {
        let name = "service-monitor";
        let socket =
            Socket::create(name).expect("service-monitor: failed to create Service Monitor scheme");

        let mut sm_scheme = SMScheme::new();

        // make list of managed services
        let mut services: HashMap<String, ServiceEntry> = read_registry();

        // start dependencies
        for service in services.values_mut() {
            let _ = start(service);
        }

        info!(
            "service-monitor daemonized with pid: {}",
            std::process::id()
        );
        daemon
            .ready()
            .expect("service-monitor: failed to notify parent");
        // TODO move dep loop here
        loop {
            eval_cmd(&mut services, &mut sm_scheme);
            // The following is for handling requests to the SM scheme
            // Redox does timers with the timer scheme according to docs https://doc.redox-os.org/book/event-scheme.html
            // not sure if that is still how it works or not, but seems similar to this code

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
                        .expect(
                            "service-monitor: failed to write responses to Service Monitor scheme",
                        );
                }
                _ => (),
            }
        }
    })
    .expect("service-monitor: failed to daemonize");
}

/// Executes then clears the command stored in the service-monitor's scheme.
fn eval_cmd(services: &mut HashMap<String, ServiceEntry>, sm_scheme: &mut SMScheme) {
    let mut result: Result<Option<TOMLMessage>, Option<TOMLMessage>>;
    match &(sm_scheme.cmd) {
        Some(SMCommand::Stop { service_name }) => {
            if let Some(service) = services.get_mut(service_name) {
                // info!("Stopping '{}'", service.config.name);
                result = stop(service);
            } else {
                warn!("stop failed: no service named '{}'", service_name);
                result = Err(Some(TOMLMessage::String(format!("Unable to stop '{}': No such service", service_name))));
            }
        }
        Some(SMCommand::Start { service_name }) => {
            if let Some(service) = services.get_mut(service_name) {
                //info!("Starting '{}'", service.config.name);
                result = start(service);
            } else {
                warn!("start failed: no service named '{}'", service_name);
                result = Err(Some(TOMLMessage::String(format!("Unable to start '{}': No such service", service_name))));
            }
        }
        Some(SMCommand::List) => {
            result = list(services)
        },
        Some(SMCommand::Clear { service_name }) => {
            if let Some(service) = services.get_mut(service_name) {
                //info!("Clearing short-term stats for '{}'", service.config.name);
                result = clear(service);
            } else {
                warn!("clear failed: no service named '{}'", service_name);
                result = Err(Some(TOMLMessage::String(format!("Unable to clear '{}': No such service", service_name))));
            }
        }
        Some(SMCommand::Info { service_name }) => {
            if let Some(service) = services.get_mut(service_name) {
                //info!("Finding information for '{}'", service.config.name);
                result = info(service);
            } else {
                warn!("info failed: no service named '{}'", service_name);
                result = Err(Some(TOMLMessage::String(format!("Unable to get info for '{}': No such service", service_name))));
            }
        }
        Some(SMCommand::Registry { subcommand }) => {
            match subcommand {
                RegistryCommand::View { service_name } => {
                    result = view_entry(service_name);
                }
                RegistryCommand::Add {
                    service_name,
                    old,
                    args,
                    manual_override,
                    depends,
                    scheme_path,
                } => {
                    let r#type = if *old { "unmanaged" } else { "daemon" };
                    // ! this overrides existing entries
                    result = add_entry(
                        service_name,
                        r#type,
                        args.as_ref().unwrap(),
                        *manual_override,
                        scheme_path,
                        depends.as_ref().unwrap(),
                    );
                    match result {
                        Ok(o) => {
                            // ! but this doesn't
                            result = add_hash_entry(
                                service_name,
                                r#type,
                                args.as_ref().unwrap(),
                                *manual_override,
                                scheme_path,
                                depends.as_ref().unwrap(),
                                services,
                            ).map(|_| o);
                        }
                        _ => {}
                    }
                }
                RegistryCommand::Remove { service_name } => {
                    result = rm_entry(service_name);
                    match result {
                        Ok(o) => {
                            result = rm_hash_entry(services, service_name).map(|_| o);
                        }
                        _ => {}
                    }
                }
                RegistryCommand::Edit {
                    service_name,
                    old,
                    edit_args,
                    scheme_path,
                    depends,
                } => {
                    result = edit_entry(
                        service_name,
                        *old,
                        edit_args.as_ref().unwrap(),
                        scheme_path,
                        depends.as_ref().unwrap(),
                    );
                    match result {
                        Ok(o) => {
                            result = edit_hash_entry(
                                services,
                                service_name,
                                *old,
                                edit_args.as_ref().unwrap(),
                                scheme_path,
                                depends.as_ref().unwrap(),
                            ).map(|_| o);
                        }
                        _ => {}
                    }
                }
            }
        },
        None => {
            // if we don't do this, writing response will crash service-monitor
            return;
        }
    }

    match result {
        Ok(msg) => {
            let _ = sm_scheme.write_response(
                &CommandResponse::new(
                    sm_scheme.cmd.as_ref().unwrap(),
                    true,
                    msg
                )
            );
        }
        Err(msg) => {
            let _ = sm_scheme.write_response(
                &CommandResponse::new(
                    sm_scheme.cmd.as_ref().unwrap(),
                    false,
                    msg
                )
            );
        }
    }

    // reset the current command value
    sm_scheme.cmd = None;
}

fn update_service_info(service: &mut ServiceEntry) {
    //info!("Updating information for: {}", service.config.name);

    let read_buffer: &mut [u8] = &mut [b'0'; 48];

    let _ = read_helper(service, read_buffer, "message");
    // grab the string
    let mut message_string = match str::from_utf8(&read_buffer[0..32]) {
        Ok(data) => data,
        Err(_) => "<data not a valid string>",
    }
    .to_string();
    // change trailing 0 chars into empty string
    message_string.retain(|c| c != '\0');
    //info!("~sm found a data string: {:#?}", message_string);
    service.message = message_string;

    let mut message_time_b: [u8; 8] = [0; 8];
    message_time_b.clone_from_slice(&read_buffer[32..40]);
    service.message_time = i64::from_ne_bytes(message_time_b);

    // get and print read, write, open, close, & dup count, they are successive u64 bytes read from requests subscheme
    let _ = read_helper(service, read_buffer, "request_count");

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

    // get and process the start time
    let _ = read_helper(service, read_buffer, "time_stamp");
    let mut time_bytes = [0; 8];
    for i in 0..8 {
        time_bytes[i] = read_buffer[i];
    }
    let time_init_int = i64::from_ne_bytes(time_bytes);
    service.time_init = time_init_int;

    service.last_update_time = Local::now().timestamp_millis();
}

fn stop(service: &mut ServiceEntry) -> Result<Option<TOMLMessage>, Option<TOMLMessage>> {
    if service.running {
        let _ = clear(service);
        info!("trying to kill pid {}", service.pid);
        let _kill_ret = syscall::call::kill(service.pid, syscall::SIGKILL);
        service.running = false;
        
        // todo: remove service from internal list if it does not exist in the registry anymore
        let name = service.config.name.clone();
        Ok(Some(TOMLMessage::String(format!("Stopped service '{}'", name))))
    } else {
        warn!("stop failed: '{}' was already stopped", service.config.name);
        Err(Some(TOMLMessage::String(format!("Unable to stop '{}': Already stopped", service.config.name))))
    }
}

fn start(service: &mut ServiceEntry) -> Result<Option<TOMLMessage>, Option<TOMLMessage>> {
    // can add args here later with '.arg()'
    if !service.running {
        match std::process::Command::new(service.config.name.as_str()).spawn() {
            Ok(mut child) => {
                //service.pid = child.id().try_into().unwrap();
                //service.pid += 2;
                service.time_started = Local::now().timestamp_millis(); // where should this go for the start command?
                // wait for the daemon loader to exit so we can safely get the pid
                match child.wait() {
                    Ok(_ext) => {}
                    Err(_) => {
                        error!("{} failed to start!", service.config.name);
                        return Err(Some(TOMLMessage::String(format!("Unable to start '{}': Process exited with failure code", service.config.name))));
                    }
                }
                let child_scheme =
                    match libredox::call::open(service.config.scheme_path.clone(), O_RDWR, 1) {
                        Ok(fd) => fd,
                        Err(_) => {
                            error!("failed to open service scheme!");
                            return Err(Some(TOMLMessage::String(format!("Unable to start '{}': Failed to open scheme at '{}'", service.config.name, service.config.scheme_path))));
                        }
                    };
                let pid_scheme = match libredox::call::dup(child_scheme, b"pid") {
                    Ok(pid_fd) => pid_fd,
                    Err(_) => {
                        error!("failed to dup service pid scheme!");
                        return Err(Some(TOMLMessage::String(format!("Unable to start '{}': Failed to dup service pid scheme", service.config.name))));
                    }
                };
                let read_buffer: &mut [u8] = &mut [b'0'; 32];
                match libredox::call::read(pid_scheme, read_buffer) {
                    Ok(_usize) => {}
                    Err(_) => {
                        error!("could not read pid from service!");
                        return Err(Some(TOMLMessage::String(format!("Unable to start '{}': Failed to read pid from service", service.config.name))));
                    }
                }
                // process the buffer based on the request
                let mut pid_bytes: [u8; 8] = [0; 8];
                for i in 0..8 {
                    //info!("byte {} reads {}", i, read_buffer[i]);
                    pid_bytes[i] = read_buffer[i];
                }
                let pid = usize::from_ne_bytes(pid_bytes);
                service.pid = pid;
                info!("child started with pid: {:#?}", service.pid);
                service.running = true;

                Ok(Some(TOMLMessage::String(format!("Started '{}' with pid {:#?}", service.config.name, service.pid))))
            }

            Err(_e) => {
                warn!("start failed: could not start {}", service.config.name);
                Err(Some(TOMLMessage::String(format!("Unable to start '{}': Failed to locate executable", service.config.name))))
            }
        }
    } else {
        //test_service_data(service);
        if &service.config.name == "gtrand2" {
            test_timeout(service);
        }
        // When we actually report the total number of reads/writes, it should actually be the total added
        // to whatever the current value in the service is, the toal stored in the service monitor is
        // updated when the service's count is cleared.
        // info!(
            //     "total reads: {}, total writes: {}",
            //     service.total_reads, service.total_writes
            // );
            
        warn!("service: '{}' is already running", service.config.name);
        Err(Some(TOMLMessage::String(format!("Unable to start '{}': Already running", service.config.name))))
    }
}

fn info(service: &mut ServiceEntry) -> Result<Option<TOMLMessage>, Option<TOMLMessage>> {
    let stats = if service.running {
        update_service_info(service);

        ServiceDetailStats {
            name: service.config.name.clone(),
            pid: service.pid,
            time_init: service.time_init,
            time_started: service.time_started,
            time_now: Local::now().timestamp_millis(),
            read_count: service.read_count,
            total_reads: service.total_reads + service.read_count,
            write_count: service.write_count,
            total_writes: service.total_writes + service.write_count,
            open_count: service.open_count,
            total_opens: service.total_opens + service.open_count,
            close_count: service.close_count,
            total_closes: service.total_closes + service.close_count,
            dup_count: service.dup_count,
            total_dups: service.total_dups + service.dup_count,
            error_count: service.error_count,
            total_errors: service.total_errors + service.error_count,
            message: service.message.clone(),
            message_time: service.message_time,
            running: service.running,
            last_update_time: service.last_update_time,
        }
    } else {
        ServiceDetailStats {
            name: service.config.name.clone(),
            pid: service.pid,
            time_init: service.time_init,
            time_started: service.time_started,
            time_now: Local::now().timestamp_millis(),
            read_count: 0,
            total_reads: service.total_reads + service.read_count,
            write_count: 0,
            total_writes: service.total_writes + service.write_count,
            open_count: 0,
            total_opens: service.total_opens + service.open_count,
            close_count: 0,
            total_closes: service.total_closes + service.close_count,
            dup_count: 0,
            total_dups: service.total_dups + service.dup_count,
            error_count: 0,
            total_errors: service.total_errors + service.error_count,
            message: service.message.clone(),
            message_time: service.message_time,
            running: service.running,
            last_update_time: service.last_update_time,
        }
    };
    Ok(Some(TOMLMessage::ServiceDetail(stats)))
}

fn list(service_map: &mut HashMap<String, ServiceEntry>) -> Result<Option<TOMLMessage>, Option<TOMLMessage>> {
    let mut service_stats: Vec<ServiceRuntimeStats> = Vec::new();

    for service in service_map.values_mut() {
        if service.running {
            update_service_info(service);
        }

        service_stats.push(ServiceRuntimeStats {
            name: service.config.name.clone(),
            pid: service.pid,
            time_init: service.time_init,
            time_started: service.time_started,
            time_now: Local::now().timestamp_millis(),
            message: service.message.clone(),
            running: service.running,
            last_update_time: service.last_update_time,
        });
    }

    Ok(Some(TOMLMessage::ServiceStats(service_stats)))
}

fn clear(service: &mut ServiceEntry) -> Result<Option<TOMLMessage>, Option<TOMLMessage>> {
    if service.running {
        // read the requests into a buffer
        let read_buffer: &mut [u8] = &mut [b'0'; 48];
        let _ = read_helper(service, read_buffer, "request_count");

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
        let _ = write_helper(service, "control", "clear");
        
        service.read_count = 0;
        service.write_count = 0;
        service.open_count = 0;
        service.close_count = 0;
        service.dup_count = 0;
        service.error_count = 0;

        Ok(Some(TOMLMessage::String(format!("Cleared short-term stats for '{}'", service.config.name))))
    } else {
        warn!("Attempted to clear '{}' which is not running!", service.config.name);
        Err(Some(TOMLMessage::String(format!("Failed to clear '{}'; service is not running", service.config.name))))
    }
}

fn test_timeout(gtrand2: &mut ServiceEntry) {
    let read_buf = &mut [b'0'; 8];
    // make sure we can read
    let _ = read_helper(gtrand2, read_buf, "");
    info!(
        "read random {:#?} from gtrand2, forcing timeout...",
        i64::from_ne_bytes(*read_buf)
    );
    let _ = write_helper(gtrand2, "", "timeout");
    // expecting this call to time out
    match write_helper(gtrand2, "", "rseed") {
        Ok(_usize) => {
            info!("wrote new seed to gtrand2 after recovering from timeout!");
        }
        Err(_) => {
            warn!("could not recover write from timeout!");
        }
    }

    let _ = write_helper(gtrand2, "", "timeout");
    match read_helper(gtrand2, read_buf, "") {
        Ok(_usize) => {
            info!(
                "read random {:#?} from gtrand2 after recovering from timeout",
                i64::from_ne_bytes(*read_buf)
            );
        }
        Err(_) => {
            warn!("could not recover from timeout!");
        }
    }
}

// todo: remove (unused)
fn test_count_ops(service: &mut ServiceEntry) -> i64 {
    let read_buf = &mut [b'0'; 8];
    let _ = read_helper(service, read_buf, "");
    info!(
        "successfully read random {:#?}",
        i64::from_ne_bytes(*read_buf)
    );
    let _ = write_helper(service, "", "");
    return i64::from_ne_bytes(*read_buf);
}

// todo: remove (unused)
fn test_err(gtrand2: &mut ServiceEntry) {
    let timeout_req = "error";
    let _ = write_helper(gtrand2, "", timeout_req);
    let read_buf = &mut [b'0'; 32];
    // for now we expect this to hang,
    match read_helper(gtrand2, read_buf, "") {
        Ok(_) => {
            // whatever happens here, do nothing, just testing
            warn!("test error failed!");
        }
        Err(_) => {
            // whatever happens here, do nothing, just testing
            info!("test error success!");
        }
    }
}

fn read_helper(service: &mut ServiceEntry, read_buf: &mut [u8], data: &str) -> Result<usize> {
    let mut try_again = true;
    let mut result: Result<usize> = Err(Error::new(EBADF));
    while try_again {
        result = match libredox::call::open(service.config.scheme_path.clone(), O_RDWR, 0) {
            Ok(child_scheme) => {
                // determine which scheme we are trying to read from
                let read_scheme = if !data.is_empty() {
                    let data_scheme = libredox::call::dup(child_scheme, data.as_bytes())?;
                    let _close_res = libredox::call::close(child_scheme);
                    data_scheme
                } else {
                    child_scheme
                };

                // read from the scheme with a timeout
                let (sender, receiver) = mpsc::channel::<Result<usize>>();
                let read_thread = thread::spawn(move || {
                    let thread_buf: &mut [u8; 64] = &mut [0; 64];
                    match sender.send(libredox::call::read(read_scheme, thread_buf)) {
                        Ok(_result) => {
                            return *thread_buf;
                        }

                        Err(_) => {
                            // must have the same return type, this return value will not be read.
                            return *thread_buf;
                        }
                    }
                });
                thread::sleep(Duration::from_millis(50));
                result = match receiver.try_recv() {
                    Ok(result) => {
                        // dropping the reciever here should intterupt the sender's thread, stop trying to read and return
                        drop(receiver);
                        read_buf.clone_from_slice(
                            &read_thread.join().expect("didn't join!?")[0..read_buf.len()],
                        );
                        let _close_res = libredox::call::close(read_scheme);
                        try_again = false;
                        result
                    }
                    Err(_recv_err) => {
                        drop(receiver);
                        warn!("read operation on {} timed out!", service.config.name);
                        // attempt to recover the service, once this returns, if the service is still running then it has ben successfully recovered
                        if recover(service) {
                            try_again = true;
                        }
                        let _close_res = libredox::call::close(read_scheme);
                        Err(Error::new(EBADF))
                    }
                };
                // if we made it here we should return an error
                result
            }
            // if we failed to open the base scheme
            _ => Err(Error::new(EBADF)),
        }
    }
    result
}

fn write_helper(service: &mut ServiceEntry, subscheme_name: &str, data: &str) -> Result<usize> {
    let mut try_again = true;
    let mut result: Result<usize> = Err(Error::new(EBADF));
    while try_again {
        result = match libredox::call::open(service.config.scheme_path.clone(), O_RDWR, 0) {
            Ok(child_scheme) => {
                // determine which scheme we are trying to read from
                let write_scheme = if !subscheme_name.is_empty() {
                    let data_scheme = libredox::call::dup(child_scheme, subscheme_name.as_bytes())?;
                    let _close_res = libredox::call::close(child_scheme);
                    data_scheme
                } else {
                    child_scheme
                };
                // read from the scheme with a timeout
                let mut thread_data: [u8; 64] = [0; 64];
                if data.as_bytes().len() < 64 {
                    thread_data[0..data.as_bytes().len()]
                        .clone_from_slice(&data.as_bytes()[0..data.as_bytes().len()]);
                } else {
                    thread_data.clone_from_slice(&data.as_bytes()[0..64]);
                }
                let (sender, receiver) = mpsc::channel::<Result<usize>>();
                let write_thread = thread::spawn(move || {
                    let thread_buf: &mut [u8; 64] = &mut [0; 64];
                    thread_buf.clone_from_slice(&thread_data[0..thread_data.len()]);
                    let mut thread_wr = thread_buf.to_vec();
                    thread_wr.retain(|c| *c != b'\0');
                    let wr: &[u8] = &thread_wr;
                    match sender.send(libredox::call::write(write_scheme, wr)) {
                        Ok(_result) => {
                            return;
                        }
                        Err(_) => {
                            // must have the same return type, this return value will not be read.
                            return;
                        }
                    }
                });
                thread::sleep(Duration::from_millis(50));
                result = match receiver.try_recv() {
                    Ok(result) => {
                        // dropping the reciever here should intterupt the sender's thread, stop trying to read and return
                        drop(receiver);
                        write_thread
                            .join()
                            .expect("write timeout thread didn't join!");
                        let _close_res = libredox::call::close(write_scheme);
                        try_again = false;
                        result
                    }
                    Err(_recv_err) => {
                        drop(receiver);
                        warn!("write operation on {} timed out!", service.config.name);

                        // attempt to recover the service, once this returns, if the service is still running then it has ben successfully recovered
                        if recover(service) {
                            try_again = true;
                        }
                        let _close_res = libredox::call::close(write_scheme);
                        Err(Error::new(EBADF))
                    }
                };
                // if we made it here we should return an error
                result
            }
            // if we failed to open the base scheme
            _ => Err(Error::new(EBADF)),
        }
    }
    result
}

fn recover(service: &mut ServiceEntry) -> bool {
    let _kill_res = syscall::kill(service.pid, syscall::SIGKILL);
    service.running = false;
    service.time_started = Local::now().timestamp_millis(); // where should this go for the start command?
    let running = match std::process::Command::new(service.config.name.as_str()).spawn() {
        Ok(mut child) => {
            let _ = child.wait();

            let child_scheme = libredox::call::open(service.config.scheme_path.clone(), O_RDWR, 1)
                .expect("couldn't open child scheme");
            let pid_req = b"pid";
            let pid_scheme = libredox::call::dup(child_scheme, pid_req).expect("could not get pid");

            let read_buffer: &mut [u8] = &mut [b'0'; 32];
            libredox::call::read(pid_scheme, read_buffer).expect("could not read pid");

            let _recover_res = match libredox::call::open(service.config.scheme_path.clone(), O_RDWR, 0) {
                Ok(child_scheme) => {
                    // determine which scheme we are trying to read from
                    let pid_scheme = {
                        let data_scheme = libredox::call::dup(child_scheme, b"pid")
                            .expect("Failed to dup id scheme during service recovery.");
                        let _close_res = libredox::call::close(child_scheme);
                        data_scheme
                    };
                    // read from the scheme with a timeout
                    let (sender, receiver) = mpsc::channel::<Result<usize>>();
                    let read_thread = thread::spawn(move || {
                        let thread_buf: &mut [u8; 64] = &mut [0; 64];
                        match sender.send(libredox::call::read(pid_scheme, thread_buf)) {
                            Ok(_result) => {
                                return *thread_buf;
                            }

                            Err(_) => {
                                // must have the same return type, this return value will not be read.
                                return *thread_buf;
                            }
                        }
                    });
                    thread::sleep(Duration::from_millis(50));
                    let final_res = match receiver.try_recv() {
                        Ok(result) => {
                            // dropping the reciever here should intterupt the sender's thread, stop trying to read and return
                            drop(receiver);
                            info!("recovered {} from timeout", service.config.name);
                            read_buffer.clone_from_slice(
                                &read_thread.join().expect("didn't join!?")[0..read_buffer.len()],
                            );
                            result
                        }
                        Err(_recv_err) => {
                            drop(receiver);
                            // for now just kill the service that timed out and return an error
                            let _ = syscall::call::kill(service.pid, syscall::SIGKILL);
                            service.running = false;
                            Err(Error::new(EBADF))
                        }
                    };
                    let _close_res = libredox::call::close(pid_scheme);
                    final_res
                }
                // if we failed to open the base scheme
                _ => Err(Error::new(EBADF)),
            };
            // process the buffer based on the request
            let mut pid_bytes: [u8; 8] = [0; 8];
            pid_bytes.clone_from_slice(&read_buffer[0..8]);
            let pid = usize::from_ne_bytes(pid_bytes);
            service.pid = pid;
            info!("child started with pid: {:#?}", service.pid);
            service.running = true;
            service.running
        }

        Err(_e) => {
            warn!("start failed: could not restart {}", service.config.name);
            service.running
        }
    };
    running
}