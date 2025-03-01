# Service Monitor
A system health monitoring service for Redox OS

## Release Notes
### Version 0.2.0

#### New Features
* The commands 'list' can be used to see a list of all registered services and some relevant data.
`services list`
* The command 'info' can be used to retrieve detailed data on a particular service.
`services info gtrand`
* The command 'clear' can be used to clear the short term data stored in a service.
`services clear gtrand`

#### Known Issues
* BaseScheme will need an additional version to support services implementing SchemBlock instead of Scheme like disk drivers.
* Moving data points around as byte arrays should be replaced with helper functions to get the byte array and translate the byte array into something useful.
* Commands to the service monitor should be of a new enum type instead of a hard coded integer.

### Version 0.1.0

#### New Features
* The service monitor starts at boot and starts its registered services
* The commands 'start' and 'stop' can be used in the command line to start and stop registered services
* The command 'list' can be used to list the PIDs of registered running services

#### Known Issues
* `smregistry.toml` should be moved to `/etc/services` in the redox directory
* Reading information from a service/scheme should be removed from any specific service (such as gtrand) to prepare for moving to `getattr/setattr` in the future


## Installation
1. `git clone` into `cookbook/recipes` folder
2. add the line `service-monitor = {}` to `[packages]` in `config/desktop.toml`
3. add `service-monitor` as the last line in `cookbook/recipes/core/initfs/init.rc` 
4. `make r.service-monitor cr.initfs desktop`

## Testing
1. launch Redox in VM and open terminal
2. `services` starts the cli tool
3. `services stop gtrand` will stop gtrand
4. `services start gtrand` will start gtrand
5. use `ps` to see running processes and `cat /scheme/sys/log` to view the log
