# Service Monitor
A system health monitoring service for Redox OS

## Release Notes
### Version 0.3.0

#### New Features
- The registry commands 'services registry ...' can be used to view and edit the registry.
    - `services registry view <daemon_name>`
    - `services registry add <--old> <daemon_name> "['arg1', 'arg2'...]" <--override> "['dep1', 'dep2'...]" <scheme_path>`
    - `services registry remove <daemon_name>`
    - `services registry edit <daemon_name> <--o> "['arg1', 'arg2'...]" <scheme_path> "['dep1', 'dep2'...]"`
    - `services** / **services --help`
    - This comes with a signifigant refactor to the way the service-monitor handles commands.

- When the service monitor attempts to read from or write to a service that is not responding it will automatically try to restart it and complete the operation.

#### Known Issues
- BaseScheme will need an additional version to support services implementing SchemBlock instead of Scheme like disk drivers.
- When attempting to run the service recovery test too quickly the whole OS will freeze. This is likely due to the threading used for timeout detection, other components may need to be refactored for multithreading for this to be fixed.
- Start and Stop commands do not give CLI feedback and should.
- Excluding the dependencies argument from `services registry edit` causes a panic
- Error text for depends list for `services registry add` prefixes the list with 'args' instead of 'depends'
- Depends list for `services registry add` is shown as optional, but omitting causes clap parser to demand it

### Bug Fixes
- The info and list commands now properly display services that are not running.

## Release Notes
### Version 0.2.0

#### New Features
- The command 'list' can be used to see a list of all registered services and some relevant data.
`services list`
- The command 'info' can be used to retrieve detailed data on a particular service.
`services info gtrand`
- The command 'clear' can be used to clear the short term data stored in a service.
`services clear gtrand`

#### Known Issues
- BaseScheme will need an additional version to support services implementing SchemBlock instead of Scheme like disk drivers.
- Moving data points around as byte arrays should be replaced with helper functions to get the byte array and translate the byte array into something useful.
- Commands to the service monitor should be of a new enum type instead of a hard coded integer.

### Version 0.1.0

#### New Features
- The service monitor starts at boot and starts its registered services
- The commands 'start' and 'stop' can be used in the command line to start and stop registered services
- The command 'list' can be used to list the PIDs of registered running services

#### Known Issues
- `smregistry.toml` should be moved to `/etc/services` in the redox directory
- Reading information from a service/scheme should be removed from any specific service (such as gtrand) to prepare for moving to `getattr/setattr` in the future


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
