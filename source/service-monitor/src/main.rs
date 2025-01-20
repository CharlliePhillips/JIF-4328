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

enum Ty {
    SM,
}

fn main() {
    let _ = RedoxLogger::new()
    .with_output(
        OutputBuilder::stdout()
            .with_filter(log::LevelFilter::Debug)
            .with_ansi_escape_codes()
            .build()
    )
    .with_process_name("SM".into())
    .enable();
    info!("SM logger started");
    
    //get arg 0 (name used to start)
    let ty = match &*std::env::args().next().unwrap() {
        "service-monitor_service-monitor" => Ty::SM,
        _ => panic!("Service monitor needs to be called as 'service-monitor_service-monitor' we prolly gotta figure out how to fix this"),
    };
        //start dependencies:
        //let _gtdemo = std::process::Command::new("gtdemo").stdout(Stdio::inherit()).spawn().expect("failed to start gtdemo");
        let mut gtrand = std::process::Command::new("gtrand").spawn().expect("failed to start gtrand");
        //let buzz = std::process::Command::new("buzz").spawn().expect("failed to start buzz");
        warn!("gtrand: {gtrand:#?}");
        
        //warn!("buzz: {buzz:#?}");
    
    redox_daemon::Daemon::new(move |daemon| {
        let name = match ty {
            Ty::SM => "service-monitor_service-monitor",
        };
        let socket = Socket::<V2>::create(name).expect("sm: failed to create Service Monitor scheme");

        // note the placeholder services vector
        let mut sm_scheme = SMScheme(ty, 0, [0; 16]);
        
        //note: this must be set (1, 1) for Service Monitor to be able to read from randd
        libredox::call::setrens(1, 1).expect("sm: failed to enter null namespace");
        daemon.ready().expect("sm: failed to notify parent");
        

        loop {
            // parse registry for updates, this could be skipped while running if no request to edit the registry is pending

            // if a new entry is found then add it to the services vector in the SM scheme
            // if there is only one entry then check if it is the placeholder and change it
            // if it's the last service being removed then replace with placeholder

            // now that the services vector is updated use the information to start the list
            //
            if sm_scheme.1 == 1 {
                let pid: usize = gtrand.id().try_into().unwrap();
                println!("trying to kill pid {pid:#?}");
                let killRet = syscall::call::kill(pid + 2, syscall::SIGKILL);
            }

            if sm_scheme.1 == 2 {
                gtrand = std::process::Command::new("gtrand").spawn().expect("failed to start gtrand");
            }

            // The following is for handling requests to the SM
            // Redox does timers with the timer scheme according to docs https://doc.redox-os.org/book/event-scheme.html
            // not sure if that is still how it works or not, but seems simmilar to this code
            // get request 
             
            let Some(request) = socket
                .next_request(SignalBehavior::Restart)
                .expect("sm: failed to read events from Service Moniotr scheme")
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
                        .expect("sm: failed to write responses to Service Monitor scheme");

                }
                _ => (),
            }
        }
    })
    .expect("sm: failed to daemonize");
}
