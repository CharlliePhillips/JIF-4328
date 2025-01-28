# Service Monitor
A system health monitoring service for Redox OS

## Release Notes
### Version 0.1.0

#### New Features
* The service monitor starts at boot and starts its registered services
* The commands 'start' and 'stop' can be used in the command line to start and stop registered services
* The command 'list' can be used to list the PIDs of registered running services

#### Known Issues
* `smregistry.toml` should be moved to `/etc/services` in the redox directory
* Reading information from a service/scheme should be removed from any specific service (such as gtrand) to prepare for moving to `getattr/setattr` in the future


# Installation and Testing
## Installation
1. `git clone` into recipes folder
2. add the line `service-monitor = {}` to `[packages]` in `config/desktop.toml`
3. add `service-monitor_service-monitor` as the last line in `cookbook/recipes/core/initfs/init.rc` 
4. `make r.service-monitor cr.initfs desktop`

## Testing
1. launch Redox in VM and open terminal
2. `service-monitor_services` starts the cli tool
3. `service-monitor_services stop service-monitor_gtrand` will stop gtrand
4. `service-monitor_services start service-monitor_gtrand` will start gtrand
5. use `ps` to see running processes and `cat /scheme/sys/log` to view the log