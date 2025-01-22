use libredox::{call::{open, read, write}, flag::{O_PATH, O_RDONLY}};
use log::{error, info, warn, LevelFilter};
use redox_log::{OutputBuilder, RedoxLogger};
use redox_scheme::{Request, RequestKind, Scheme, SchemeBlock, SchemeBlockMut, SchemeMut, SignalBehavior, Socket, V2};
use std::{borrow::BorrowMut, fmt::{format, Debug}, fs::{File, OpenOptions}, io::{Read, Write}, os::{fd::AsRawFd, unix::fs::OpenOptionsExt}, process::{Command, Stdio}};
use scheme::{SMScheme};
use timer;
use chrono;
use std::sync::mpsc::channel;
mod scheme;


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
    

    //start dependencies, should they be stored as a list/vector of 'process::Child'?
    let mut gtrand = std::process::Command::new("gtrand").spawn().expect("failed to start gtrand");
    // TODO make this condition part of the list/vec of services
    let mut gtrand_r: bool = true;
    
    info!("started gtrand with pid: {:#?}", gtrand.id() + 2);
    
    redox_daemon::Daemon::new(move |daemon| {
        let name = "service-monitor";
        let socket = Socket::<V2>::create(name).expect("service-monitor: failed to create Service Monitor scheme");

        // note the placeholder services vector
        let mut sm_scheme = SMScheme(0, [0; 16]);
        
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
            if sm_scheme.0 == 1 && gtrand_r {
               let mut pid: usize = gtrand.id().try_into().unwrap();
                //for some reason the pid from 'ps' is different (normally 2 higher) than .id() returns?
                pid += 2;
                info!("trying to kill pid {pid:#?}");
                let killRet = syscall::call::kill(pid, syscall::SIGKILL);
                gtrand_r = false;
            } else if sm_scheme.0 == 1 {
                warn!("gtrand is already stopped");
            }
            // start: check if service is running, if not build command from registry and start
            if sm_scheme.0 == 2  && !gtrand_r {
                // can add args here later with '.arg()'
                gtrand = match std::process::Command::new("gtrand").spawn() {
                    Ok(child) => {
                        info!("child started with pid: {:#?}", child.id() + 2);
                        gtrand_r = true;
                        child
                    },
                    
                    Err(e) => {
                        warn!("could not start gtrand");
                        gtrand
                    }
                };
            } else if sm_scheme.0 == 2 {
                warn!("gtrand is already running");
            }
            //reset the current command value
            sm_scheme.0 = 0;


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
