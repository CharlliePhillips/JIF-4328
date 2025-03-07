- Feature Name: service-monitor
- RFC Date: Oct 25, 2024
- Start Date: (fill me in with today's date, YYYY-MM-DD)
- RFC PR: (leave this empty)
- Redox Issue: (leave this empty)

# Summary
We will be creating a Redox OS service to monitor other services, a protocol to communicate with those services, a library for further development, a CLI, and possibly a GUI to aid in monitoring the health of device drivers and other system services (daemons). The goal is for Redox OS to be able to recover from many software and hardware failures automatically. Throughout the process, we will create an analysis of which daemons can/cannot be recovered successfully, and what design changes are required for Redox to improve recoverability. 


# Motivation
It is critical that a way to monitor and recover services and other daemons be implemented. Doing so will eliminate issues with data loss, system instability, and device compatibility. Rolling this functionality in with the ability to start other programs in an unmanaged way will allow for the service monitor to replace the current init system. Proper design and implementation will help lay the groundwork for further development in other parts of Redox OS as well. 

# Detailed design
## User stories 

-  System Start (assume all devices and daemons are known) 
    - When the Services Manager is started at the end of the boot process it will open the `registry.toml` and read the list of services and their dependent daemons to build a dependency tree. It will then start them in parallel starting at the root(s) (or as each service’s dependencies become available). Any arguments to those programs that would normally be passed to the command line will be specified in an arguments array for each service in the registry. The specified Type of the service can tell the Services Manager how it will treat the service. For example, some services (old style daemons) may be started by the Service Manager but otherwise will not interact with it.  

- Device Discovery (previously unknown device added to registry) 
    - Long term the Redox team would like to add a “device discovery” daemon that would look for devices attached to the computer and determine their dependencies. This discovery daemon would then request any services needed for a device be started. These requests will be received by the service monitor which will start that service as specified in the registry.toml. While that start thread is running the registry file will be checked for the service and the parameters will be updated if they are different and the manual parameter flag is not set (see more questions). The device discovery daemon is a future project and may need other functionality like stopping, registering, or de-registering services. There should be an API for these kinds of requests as well to make room for future development. 

- Timer-based Status Check 
    - After a certain time interval, the SM will run a thread to check each managed daemon, read the active bit, message, and calculate the uptime, this data will be sent to the log daemon for recording. 

- Timer-based Failure Detection and Restart 
    - The timer-based status check can detect a failing daemon if the managed service’s active bit is 0, or it fails to respond with valid data on the regular interval, then the service should be stopped and restarted. Some services will have information in the kernel that is required to properly restart them. What syscall(s) this will be? 

### User Commands: 
A separate program with the name “services” will parse the arguments passed and call the Service Monitor API accordingly to provide a user interface to the daemon. The services CLI application will open the service monitor scheme and reference it with a file descriptor. The Service Monitor API will use the getattr & setattr syscalls with to send and recieve information from the CLI application. While the getattr & setattr calls are still in development the read/write syscalls will be used. A GUI alternative should also be relatively easy to build with the same Service Monitor API and libcosmic.  

#### Stories for each user command:
1. **services list:**
    - lists all registered daemons, their current status/message string, pid, and uptime. 
    - How to list running vs not running services? 
    should show something kinda like this in the CLI:
    ```
    name | pid | uptime | message | state
    
    gtrand | 85 | 3m 40s | “<random num>” | RUNNING
    ```
    - If a daemon is stopped or otherwise not responding:
    ```
    name | pid | uptime | message | state

    gtrand2 | N/A | N/A | N/A | STOPPED 
    ```
    - If a service has been restarted then that should be indicated when requesting information from the service in the CLI, this example shows a service that was restarted 4 minutes and 43 seconds ago.
    ```
    name | pid | uptime | message | state

    gtrand2 | 88 | 4m 43s | "<random num>" | RESTARTED
    ```
2. **services info <daemon_name>:** 
    - list the current status, info, and uptime for <daemon_name>  
        - Uptime – difference between the current time and the start time that the daemon has recorded. 
        - Last time to init - The SM records when it starts a thread to start a service and the service records when it is done initializing, this difference is recorded as the last time to initialize. 
        - Total number of requests (read/write) as well as # since last clear 
        - Scheme size 
        - Total # of errors logged 
        - Last response time – The last time that the daemon was responsive for timeouts.
        - **example:**
        ```
        user:~$services info gtrand
        Service: gtrand
        Uptime: 0 hours, 0 minutes, 35.923 seconds
        Last time to initialize: 0 minutes, 0.404 seconds
        Read count: 13
        Write count: 42
        Open count: 2
        Close count: 1
        Dup count: 2
        Error count: 0
        Message: "randum no: -1742356297751"
        user:~$
        ``` 
    - **What info shows if a daemon is not responding?**
        - If a daemon is stopped or otherwise not responding:
            `service "<service_name>" is stopped!`
            - The service monitor will store the current state of each service as an enum `RUNNING`, `STOPPED`, `RESTARTED`. When the service monitor starts a service, or clears the data on a service marked as `RESTARTED` it will be put in the `RUNNING` state. 
            - If the service has been stopped or otherwise becomes unresponsive it will be marked as being in the `STOPPED` state and will not be started again unless the service monitor recieves an external request to restart it.
            - If a service has been restarted then that should be indicated when requesting information from the service in the CLI, this example shows a service that was restarted 4 minutes and 43 seconds ago.
            ```
            user:~$services info gtrand2
            Service: gtrand2
            RESTARTED!
            Uptime: 0 hours, 4 minutes, 43.000 seconds
            Last time to initialize: 0 minutes, 0.212 seconds
            ... // see example above for other info
            ``` 
            - If the daemon is providing any specific failure message, that should be listed along with statistics that indicate to the service manager that it is failing.
            - Rust example of how a service could be checked as unresponsive: 
            ```rust
            let (sender, receiver) = mpsc::channel();
            let t = thread::spawn(move || {
                match sender.send(Ok(read(fd, read_buffer))) {
                    Ok(()) => {}, // everything good
                    Err(_) => {}, // sender has been released, don't panic
                }
            });
            // this line will block until the read is complete or 5 seconds has passed, whichever comes first.
            let result = receiver.recv_timeout(Duration::from_millis(5000));
            ```
            - This timeout code can be used with the open, read, write, & dup syscalls in helper functions of the service monitor to prevent the calls from hanging and consequently hanging the service monitor. If the reciever times out then it will return an error attempt to restart it.
            - If it is restarted then the restart time is recorded by service monitor, if we attempt to restart this service again within 5 seconds of this recorded time then the service is marked as unresponsive. An unresponsive service will have the state `STOPPED` and will not be restarted.
            - This does leave a question open on if we are unable to open and read the pid from a service we just started then should we assume it failed to start?
            - more info [in the rust docs](https://doc.rust-lang.org/std/sync/mpsc/fn.channel.html).
 3. **services clear <daemon_name>:**
- Clear short-term stats for <daemon_name>. 
    - A user could clear short term stats and monitor for unusual changes (say a process is not using io when it normally should) This change in short term info can then be used to determine issues with the daemon A similar flow will be implemented as an automated part of the service manager. 
        - Requests count – Total requests are still recorded by Service Manager 
        - Message – Set service’s message to placeholder “Message Cleared” 
        - Errors – The service’s error list and count is cleared/set to 0. 
        - Last response time & timeout – This is recorded per service, but by the Service Monitor. 
4. **services start <daemon_name>:** 
    - Starts registered daemon with the default arguments and settings specified in the `registry.toml`. If the daemon is already running inform the user and do nothing. If any services that daemon depends on are not found/running, then the user is informed of the missing dependencies, and nothing is done. To automatically start any dependent services, add the `-f / -force` argument. 
5. **services stop <daemon_name>:**
    - Stops the registered daemon. First by “asking nicely” via setting a value in that daemon via the `setattr()` syscall. Then by sending a hang up signal (SIGHUP), and if the daemon is still running, by sending a kill signal (SIGKILL). Each syscall will be handled on its own thread, and should the operation take too long to return an alarm signal (SIGALRM) would be sent. This avoids the potential of the entire service manager getting caught on an unresponsive service. 
        
        services stop <daemon_name>: 
        ```rust
        let service = File::open(“/scheme/<scheme_name>”) 

        setattr(service.as_raw_fd, “stop”, 1) 

        // wait for daemon to try nicely then try harder 
        If getattr(service.as_raw_fd, “active”) { 
            signal::kill(<daemon PID>, Signal::SIGHUP).unwrap(); 
        } 

        // wait again 
        If getattr(service.as_raw_fd, “active”) { 
            signal::kill(<daemon PID>, Signal::SIGKILL).unwrap();
        }
        ```
    - Restart and Restore: 
        - Adding the `-restart` argument stops a registered service and then starts it. Long term data from a managed daemon scheme should be recorded. Some services require information from the kernel to be started in the correct state after Redox has booted. For these services use the argument `–restore`. Ex: `services stop –restore <daemon_name>`
6. **services register <daemon_name> args=[] path="/scheme/<daemon_name>" depends=["other_daemon"]:** 
    - Adds an entry for a daemon into the list of managed services, it will be started by the SM with the command line args specified in the array. To manually register an old-style daemon for the SM to start but ignore (i.e. use the SM as init), a user could enter the command `services register –o <daemon_name> args=[]...` where the application path is a valid path to a binary (or the name of one on PATH?). The registry may need additional API calls for editing existing services’ info, we will need to decide if/how this will be controlled by arguments or additional commands.
    - `services register -rm <daemon_name>` 
    - `services register -edit <daemon_name> -o args=[] path="/scheme/<daemon_name>" depends=["other_daemon"]` takes variable arguments after `<daemon_name>` to update the registry entry for a service.
    - for arguments `-rm` and `-edit` If that service is running when we attempt to edit the registry then nothing should be done and the user notified that the service cannot be changed while running.
    - `services register -info <daemon_name>` show the registry entry for the specified service.
7. **services** / **services --help**
    - Displays a help page detailing the available commands 

## APIs and Message Flows 
#### Managed Service API (new-style daemons)
- Each managed service will have it's main/primary scheme attached to a `BaseScheme` containing several sub-schemes that hold managment data. The BaseScheme will present the main/primary scheme the same way it would be accesed if it was not managed. Data from these managment schemes can be accessed by calling `dup` on the service's scheme and then `read` or `write` on the resulting file descriptor.

- The sub-schemes of BaseScheme are:
    - `main_scheme` - The primary scheme for the service, or the one that is pre-existing for an old-style daemon being converted to a managed one.
    - `pid_scheme` - Contains a u64 proccess id obtained from std::process
    - `requests_scheme` - Holds five integers counting requests to the main scheme for read, write, open, close, and dup
    - `time_stamp_scheme` - Holds a 64 bit timestamp of when the service was started. Recorded in seconds since Unix epoch (1/1/1970).
    - `message_scheme` - Holds a 32 byte array of charachters for a human readable status message.
    - `control_scheme` - Holds a bool to indicate if a clear has been requested by the service monitor and another to indicate a graceful shutdown has been requested.

- The each of the BaseScheme sub-schemes wrapped in the type `ManagmentSubScheme`. This type is an alias for `Arc<Mutex<Box<dyn Scheme>>>` which allows different sized structs implementing Scheme to be accessed in a threadsafe way as the same type.

- BaseScheme handles access to it's subschemes via a hash-map with open ids as the key and an `ManagmentSubScheme` as it's value. When trying to access a scheme through the BaseScheme a function `handler(id: usize)` is called to get a thread lock on that reference. The scheme's methods can then be called on this mutex guard thanks to Rust's deref trait.

- The BaseScheme implements the following methods from Scheme:
    - `xopen` - Opens the main scheme and adds the new fd and a clone of the arc-mutex containing the scheme to the hash-map of handlers.
    - `dup` - 
        1. If the hash-map does contain the id passed as key then return `EBADF`.
        2. If the map does contain the key and nothing is passed on the buffer then the dup call is forwarded to the main scheme to get a new id to be added to the hashmap. 
        3. If the buffer contains the name of a managment scheme then a new id is assigned for that scheme and added to the hash-map. 
        4. If the id passed is recognized but the information on the buffer is not then the scheme associated with that id is forwarded the dup call and the new id from that is added to the map.
        5. The new id is wrapped in Result and returned
    - `read` - Gets the handler associated with the passed id using `handle()` and passes the read call to that scheme. If the handler belongs to the main scheme then this acess will be counted.
    - `write` - Works the same as read but passes the write call to a subscheme.
    - `close` - Checks if the passed id is in the hashmap, if it is then pass the close call to the subscheme. The hashmap entry is removed regardless of if calling close on the subscheme was successful.
    - The other methods in the Scheme trait implementation for BaseScheme (fcntl, fsync, etc.) will forward to calling on the main scheme.

- The main scheme for each service will implement the ManagedScheme trait. This trait will contain a collection of methods used by the BaseScheme trait to track the main scheme's statistics. Each of the managment sub-schemes will also implement ManagedScheme so that it's methods may be called on any scheme handlers in the BaseScheme.
    - `count_ops() -> bool`: returns true if file operations (read, write, open, close, & dup) on this scheme should be counted in the BaseScheme statistics
    - `message -> Option<[&u8; 32]>` - Returns an Option containing a new 32 btye status message or None if a new message is not available.
    - `shutdown()` - gracefully stops service, closing open fds, clean up, etc. This is called at an appropriate time in the BaseScheme when ControlScheme.stop is true.

- The BaseScheme also contains a managment structure wrapped in an arc mutex. This managment structure contains the recorded statistics for a particular service
```rust
pub struct Managment {
    pid: usize,
    time_stamp: i64,
    message: [u8; 32],
    read_count: u64,
    write_count: u64,
    open_count: u64,
    close_count: u64,
    dup_count: u64,
}
```
- **Note:** for main schemes implementing the SchemeBlock trait a different BaseScheme and Managment struct will be neccecary for tracking things such as delay time. SchemeBlock allows IO calls to take their time to complete for handling things like drive access. This kind of flow would appear to be an error for the standard BaseScheme.

### Service Status, Failure Detection & Recovery
- Each service/daemon in redox has a scheme associated with it where data is stored. This scheme can be accessed as a file with the `open` syscall when passed the correct path. The file descriptor from a fully managed service can be passed to the `dup` syscall along with a byte array containing the name of the desired managment data in order to get a file descriptor pointing to that data. 
ex:
```rust
let child_scheme = libredox::call::open(service.scheme_path.clone(), O_RDWR, 0).expect("failed to open chld scheme");
// dup into the pid scheme in order to read that data
if let Ok(pid_scheme) = libredox::call::dup(child_scheme, b"pid") {
// now we can read the pid onto the buffer from it's subscheme
    let mut read_buffer: &mut [u8] = &mut [b'0'; 32];
    libredox::call::read(pid_scheme, read_buffer).expect("could not read pid");
    ...
```

- While getattr and setattr are still in development the service monitor will use the `read` syscall on a file descriptor pointing to a scheme containing the desired data. When the service recieves this request it finds the scheme associated with that fd and transparently calls read on that particular scheme. The requested data is written to the buffer passed to read for processing. 

- The `write` syscall can be used to modify particular managment sub-schemes.
ex:
```rust
let child_scheme = libredox::call::open(service.scheme_path.clone(), O_RDWR, 0).expect("failed to open chld scheme");
if let Ok(message_scheme) = libredox::call::dup(child_scheme, b"message") {
    libredox::call::write(message_scheme, b"A new message!")...
```

- The file descriptor(s) and registry.toml info for each monitored service is used with the protocols below to collect data on each service. This will then be used to restart or restore processes when they are not working correctly.

- Protocalls here are a 32-byte string passed to getattr()/setattr() with a file descriptor of the service to request statistics from. The file descriptor is obtained by opening the service’s scheme path as a file. A managed service’s scheme will get one of these strings in it’s get/setattr and match it to a function that is part of the managed scheme trait to read and/or write the relevant data to/from the scheme. While getattr and setattr are being implemented read and write will be used instead. 
    - `active` Boolean indicates if a service is running, it is set to false when read, and set back to true by the service if it is still running. 
    - `time_stamp` Unix timestamp of when service started. 
    - `message` An X byte limit string with a human readable message indicating the state of the service. Errors are logged to ‘error_list’ 
    - `stop` When called the daemon will attempt to shut down gracefully potentially preserving state for restarting. 
    - `request_count` How many requests received in a tuple of ints (read, write, open, close, dup) 
    - `error_count` size of error_list. 
    - `error_list` list of recorded error messages. 
    - `clear` set to 1 to clear the short-term statistics

- There are additional protocalls for services using SchemeBlock
    - `average_delay`
    - `last_delay`
    - WIP

- Threaded wrappers for get/setattr() will also be needed in the case that a thread executing either syscall is hung on that command. These are interruptible and will return EINTR when their running thread is signaled with SIGALRM. Another thread will monitor the wrapper(s) for response times that are too long and signal them with SIGALRM (using pthread_kill) if they exceed some timeout period. 

- There is some data that will be stored in the Service Monitor for each service running: 
    - Total number of requests (this ignores clearing) 
    - Total # of errors logged (this ignores clearing) 
    - Last response time 
    - Timeout – when true, the last response time exceeded the timeout limit.

### Command Line & Service Monitor API 
- The command line application described above will have corresponding API calls to the Service Monitor daemon that could also be called by future OS components. These are accessed by using the `write` syscall to make a request to the service monitor and `read` to read a response.
- When the service monitor recieves a request from the `write` call it will place the appropriate `service-monitor::CMD` enum into it's scheme. Inside of the service-monitor's main loop, the `eval_cmd()` function is called, which matches the command enum to it's cooresponding function and passes any associated data (arguments) along with the funciton call.
- Each function handling an API command will write data to a vector of bytes in the service-monitor scheme. When `read` is called on the service-monitor next, the requested command's vector is written to the buffer passed in the read call if applicable (if the API call return value needs more than a single usize).

### Service Start (legacy/old-style daemons) 
- Use rust standard library to build a command that starts the daemon. Once it is started, the Service Manager does not need to do anything. 

### Service Start (new-style daemons) 
- Same as legacy, but the file descriptor for the new daemon is recorded by the SM to reference later.

## Service Registry 
- Uses and Flows 
    - The Services Manager’s `registry.toml` is updated by Service Discovery through the service manager’s API.  This will run at boot before the rest of the Service Manager is started. For development it should also be possible to enter information into `registry.toml` manually. The`registry.toml` will contain information on how to start a service and what the Service Manger’s behavior will be while managing it. 
        - There will be at least one driver (ACPI-AML) that can only run once during startup and cannot safely be restarted. It will need to be monitored to see if it fails, but the registry will need to include something like, "monitor but don't restart". 
    - While the Services Manager is running a user can manually add a service by entering the `services register` command. 

- Format 
    - The `registry.toml` stores the commands and arguments to start a service in a .toml file. Each service should have:  
      - A service heading 
      - Name 
      - Type - You could specify for a service to be ignored by the SM (i.e. using SM as init) by setting the Type to “application”. 
      - Starting Arguments 
      - Manual Override – If you enter custom data into the registry.toml and do not want the Service Monitor to potentially override it then this should be set to true. Otherwise risk this information being “corrected” 
      - Depends – A list of named dependencies, this list is used to build dependency tree(s)
      - Scheme Path – path to the scheme associated with the service
    - In addition, a default `running` state of false and `pid` of 0 is assigned to the service when it is read into the manager, which is updated later on.

Example: 
```toml
[[service]]
name = "<service>"
type = "daemon"
args = [ "0" ]
manual_override = true
depends = []
scheme_path = "/scheme/<service>"
```

## Design Overview 

1. **Startup**
    - This program is intended to replace the current init process and will run early in the boot process. It will use the ‘registry.toml’ to determine how to start these processes and in what order by their dependencies. This could be modified later to accommodate a device discovery daemon that validates the registry and or modifies it before or while the service monitor begins starting other services. 

2. **Running loop**
    - The daemon loop should begin monitoring as the first service has started. The startup should probably be a child thread of the main loop so it could also be used for starting new services after boot is complete. On some time period the service monitor will check each of it’s client services for a new message or errors, request count, and a response time will be recorded. This information will then be used to report and potentially recover from any service failures. The service monitor will also handle API requests which may require data from and additional requests to the client services. 

3. **API**
    - An API will be provided to the Service Monitor via `read` and `write` on it’s scheme (until get and setattr are ready). Calls when implemented will retrieve information recorded in the Running loop for whatever is program is calling them, or trigger the Service Monitor to start/kill a service. This API will allow code to be triggered by the getattr/setattr syscalls from other applications.
    - A managment API will be provided for each managed scheme through a `BaseScheme`. This struct holds the primary scheme for the service as well as several others containg the data needed for the service monitor. The `read` and `write` syscalls will be used to access these sub-schemes until `getattr` and `setattr` are ready. The primary scheme will be accessable through the BaseScheme using the same convention as before to ensure compatibility with existing code.

4. **CLI**
    - A relatively simple program to serve as the user interface that parses command line arguments and makes the corresponding getattr/setattr calls to the Service Monitor API. It then will take any information from the Service Monitor and format it to be printed for the user. 

5. **Further Development**
    - The API should allow for easy development of a GUI application, as well as an interface for a future device discovery daemon that would modify ‘registry.toml’ through the Service Manager. Additional code could be added to control services that also have the ‘service-monitor’ trait, this might lend well to this program serving as a template for more specific monitors such as for USB devices. A USB monitor could assume certain dependencies are available and begin starting devices in it’s own registry with it’s own connection to the device discovery daemon modeled after the API described here. 

# Drawbacks
- Registry could become giant unorganized text file 
- How to handle multiple instances of the same service? 
- Domain specific service monitors will likely require additional custom code or API calls to be created.
- With the current implementation if the service monitor writes a request and to a service and then another program attempts to read from that service then it will recieve the response intended for the service manager.

# Alternatives
- Keeping old init script vs. using the service monitor as init. The service registry should be able to start registered applications without having them stay monitored for those that do not support it. This Service Monitor is intended to replace init and incrementally add services to be monitored. 
- Making the registry split up among multiple files, maybe one per service 

# Unresolved questions
- With the current implementation if the service monitor writes a request and to a service and then another program attempts to read from that service then it will recieve the response intended for the service manager. How can we tell from inside the `read` function what process called it? Will we have to store something in the `Managment` struct to help identify the service_monitor process?
- Any remaining common protocols and device specific protocols 
- Should the timestamp use seconds milliseconds?
- Should the Device Discovery remove formerly discovered services or manually added services that aren’t found for stability? 
- Daemon dependencies will come from `Cargo.toml/.lock`? `Registry.toml`? 
- What happens when a discovered service exists in the registry but the parameters discovered are different then those in the registry, update? Will we need an additional flag in the registry for manual override of this update? 
- Should a file descriptor for the child's BaseScheme be recorded, the base scheme and managment descriptors? Or should the service monitor open and close file descriptors as it runs.
- Thread safe function wrappers for getattr/setattr. One thread monitoring all wrappers or a monitor thread for each wrapper? How long, how would a user configure timeout time, should they? 
- Automatic restart – triggered on faulting daemon. Need to consider how to detect service “bootloop” to prevent dead service hogging resources. 
- For ‘not responding’  how many times and how short of a time period? Is this something determined by daemon, historic data, arbitrary numbers to be manually tuned for now? 
- Implement a "discovery" protocol that adds devices discovered during boot? 
- When the ‘start’ command is used from the CLI should nothing be done if missing dependencies? Or should those be started automatically? 
- Which daemons need what information from the Kernel, what syscall? 
- What happens when a service is not responding that has dependent services still running? 
- How do permissions/security on the API work? 
- if we are unable to open and read the pid from a service we just started then should we assume it failed to start?
