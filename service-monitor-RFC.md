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

2. **services info <daemon_name>:** 
    - list the current status, info, and uptime for <daemon_name>  
        - Uptime – difference between the current time and the start time that the daemon has recorded. 
        - Last time to init - The SM records when it starts a thread to start a service and the service records when it is done initializing, this difference is recorded as the last time to initialize. 
        - Total number of requests (read/write) as well as # since last clear 
        - Scheme size 
        - Total # of errors logged 
        - Last response time – The last time that the daemon was responsive for timeouts. 
    - **What info shows if a daemon is not responding?**
        - If a daemon is not responding: 
            - A daemon marked as not responding if it has been restarted too many times in too short of a time period. If the daemon is providing any specific failure message, that should be listed along with statistics that indicate to the service manager that it is failing. 
            - For example, if we know a daemon has timed out, we could clear the data for that service to see if it times out again before stopping/restarting. 
            - Pseudo-rust example of how this is checked: 
            ```rust
            Let response = empty; 
            Let resopnse_opt = getattr(daemon, “message”) 
            Let response_thread = thread.spawn({ 
            //will change response once one from the syscall is ready 
                response = response_opt.unwrap(); 
            }); 
            Let response_timeout = thread.spawn({ 
                timer.delay(TIMEOUT_CONSTANT); 
                While timer.waiting && response.is_empty{ 
                timeout(dameon, true) 
                } 
                If timer.waiting { 
                timeout(daemon, false) 
                //record response time 
                } 
            }); 

            response_timeout.join 
            singal::kill(response_thread) // if still running 
            // we may have to change how the response thread behaves depending on how getattr/setattr is encapsulated in it’s own thread 
            // we can now move forward with the response data, or handle a service timeout like described above 
            ```
            - Review “APIs and message flows” for specifics on how to implement this 
 3. **services clear <daemon_name>:**
Clear short-term stats for <daemon_name>. 
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
6. **services register <daemon_name> args=[]:** 
    - Adds an entry for a daemon into the list of managed services, it will be started by the SM with the command line args specified in the array. To manually register an old-style daemon for the SM to start but ignore (i.e. use the SM as init), a user could enter the command `services register –o <daemon_name> args=[]` where the application path is a valid path to a binary (or the name of one on PATH?). The registry may need additional API calls for editing existing services’ info, we will need to decide if/how this will be controlled by arguments or additional commands. 

## APIs and Message Flows 
### Command Line & Service Monitor API 
- The command line application described above will have corresponding API calls to the Service Monitor daemon that could also be called by future OS components. This will work similarly to the protocols for other services described below, but with the behavior described above. 
### Service Start (legacy/old-style daemons) 
- Use rust standard library to build a command that starts the daemon. Once it is started, the Service Manager does not need to do anything. 
### Service Start (new-style daemons) 
- Same as legacy, but the file descriptor for the new daemon is recorded by the SM to reference later.  
### Status, Failure Detection & Recovery 
- File descriptor and registry.toml info for each monitored service is used with the protocols below to collect data on each service. This will then be used to restart or restore processes when they are not working correctly 

- Protocalls here are a 32-byte string passed to getattr()/setattr() with a file descriptor of the service to request statistics from. The file descriptor is obtained by opening the service’s scheme path as a file. A managed service’s scheme will get one of these strings in it’s get/setattr and match it to a function that is part of the managed scheme trait to read and/or write the relevant data to/from the scheme. While getattr and setattr are being implemented read and write will be used instead. 
    - `active` Boolean indicates if a service is running, it is set to false when read, and set back to true by the service if it is still running. 
    - `time_stamp` Unix timestamp of when service started. 
    - `message` An X byte limit string with a human readable message indicating the state of the service. Errors are logged to ‘error_list’ 
    - `stop` When called the daemon will attempt to shut down gracefully potentially preserving state for restarting. 
    - `request_count` How many requests received in a tuple of ints (read, write) 
    - `error_count` size of error_list. 
    - `error_list` list of recorded error messages. 
    - `clear` set to 1 to clear the short-term statistics 

- Threaded wrappers for get/setattr() will also be needed in the case that a thread executing either syscall is hung on that command. These are interruptible and will return EINTR when their running thread is signaled with SIGALRM. Another thread will monitor the wrapper(s) for response times that are too long and signal them with SIGALRM (using pthread_kill) if they exceed some timeout period. 
- Each service/daemon in redox has a scheme associated with it where this data will be stored. They will be added as traits to ‘redox-scheme’. 
- There is some data that will be stored in the Service Monitor for each service running: 
    - Total number of requests (this ignores clearing) 
    - Total # of errors logged (this ignores clearing) 
    - Last response time 
    - Timeout – when true, the last response time exceeded the timeout limit. 

## Service Registry 
- Uses and Flows 
    - The Services Manager’s `registry.toml` is updated by Service Discovery through the service manager’s API.  This will run at boot before the rest of the Service Manager is started. For development it should also be possible to enter information into `registry.toml` manually. The`registry.toml` will contain information on how to start a service and what the Service Manger’s behavior will be while managing it. 
        - There will be at least one driver (ACPI-AML) that can only run once during startup and cannot safely be restarted. It will need to be monitored to see if it fails, but the registry will need to include something like, "monitor but don't restart". 
    - While the Services Manager is running a user can manually add a service by entering the `services register` command. 

- Format 
    - The `registry.toml` stores the commands and arguments to start a service in a .toml file. Each service should have (e.g.)  
    - A service heading 
    - Name 
    - Type - You could specify for a service to be ignored by the SM (i.e. using SM as init) by setting the Type to “application”. 
    - Starting Arguments 
    - Manual Override – If you enter custom data into the registry.toml and do not want the Service Monitor to potentially override it then this should be set to true. Otherwise risk this information being “corrected” 
    - Depends – A list of named dependencies, this list is used to build dependency tree(s) 

Example: 
```toml
[service] 

Name = “zerod” 

Type = “daemon” 

Args = [] 

Manual_Override = false 

Depends = [] 
```

## Design Overview 

1. **Startup**
    - This program is intended to replace the current init process and will run early in the boot process. It will use the ‘registry.toml’ to determine how to start these processes and in what order by their dependencies. This could be modified later to accommodate a device discovery daemon that validates the registry and or modifies it before or while the service monitor begins starting other services. 

2. **Running loop**
    - The daemon loop should begin monitoring as the first service has started. The startup should probably be a child thread of the main loop so it could also be used for starting new services after boot is complete. On some time period the service monitor will check each of it’s client services for a new message or errors, request count, and a response time will be recorded. This information will then be used to report and potentially recover from any service failures. The service monitor will also handle API requests which may require data from and additional requests to the client services. 

3. **API**
    - An API will be provided to the Service Monitor daemon via an additional trait for it’s scheme. These traits when implemented will retrieve information recorded in the Running loop for whatever is program is calling them, or trigger the Service Monitor to start/kill a service. This API will allow code to be triggered by the getattr/setattr syscalls from other applications. 

4. **CLI**
    - A relatively simple program to serve as the user interface that parses command line arguments and makes the corresponding getattr/setattr calls to the Service Monitor API. It then will take any information from the Service Monitor and format it to be printed for the user. 

5. **Further Development**
    - The API should allow for easy development of a GUI application, as well as an interface for a future device discovery daemon that would modify ‘registry.toml’ through the Service Manager. Additional code could be added to control services that also have the ‘service-monitor’ trait, this might lend well to this program serving as a template for more specific monitors such as for USB devices. A USB monitor could assume certain dependencies are available and begin starting devices in it’s own registry with it’s own connection to the device discovery daemon modeled after the API described here. 

# Drawbacks
- Registry could become giant unorganized text file 
- How to handle multiple instances of the same service? 
- Domain specific service monitors will likely require additional custom code or API calls to be created. 

# Alternatives
- Keeping old init script vs. using the service monitor as init. The service registry should be able to start registered applications without having them stay monitored for those that do not support it. This Service Monitor is intended to replace init and incrementally add services to be monitored. 
- Making the registry split up among multiple files, maybe one per service 

# Unresolved questions
- Any remaining common protocols and device specific protocols  
- Should the Device Discovery remove formerly discovered services or manually added services that aren’t found for stability? 
- Daemon dependencies will come from `Cargo.toml/.lock`? `Registry.toml`? 
- What happens when a discovered service exists in the registry but the parameters discovered are different then those in the registry, update? Will we need an additional flag in the registry for manual override of this update? 
- Thread safe function wrappers for getattr/setattr. One thread monitoring all wrappers or a monitor thread for each wrapper? How long, how would a user configure timeout time, should they? 
- Automatic restart – triggered on faulting daemon. Need to consider how to detect service “bootloop” to prevent dead service hogging resources. 
- For ‘not responding’  how many times and how short of a time period? Is this something determined by daemon, historic data, arbitrary numbers to be manually tuned for now? 
- Implement a "discovery" protocol that adds devices discovered during boot? 
- When the ‘start’ command is used from the CLI should nothing be done if missing dependencies? Or should those be started automatically? 
- Which daemons need what information from the Kernel, what syscall? 
- What happens when a service is not responding that has dependent services still running?s 
- How do permissions/security on the API work? 