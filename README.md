# Service Monitor
A system health monitoring service for Redox OS

## Release Notes

### Version 1.0.0
#### Features
* The service-monitor can start daemons and manage those that use the `BaseScheme` API.
* GUI is available for quick access to a list of services, statistics, and controls.
`services-gui`
* Help option in command line, add `-h` or `--help` to any command for usage information
* The command `list` can be used to see a list of all registered services and some relevant data.
`services list`
* The command `info` can be used to retrieve detailed data on a particular service.
`services info gtrand`
* The command `clear` can be used to clear the short term data stored in a service.
`services clear gtrand`
* The service monitor starts at boot and starts its registered services
* The commands `start` and `stop` can be used in the command line to start and stop registered services
* The command `list` can be used to list the PIDs of registered running services
* The registry commands `services registry ...` can be used to view and edit the registry.
    - `services registry view <daemon_name>`
    - `services registry add <--old> <daemon_name> "['arg1', 'arg2'...]" <--override> "['dep1', 'dep2'...]" <scheme_path>`
    - `services registry remove <daemon_name>`
    - `services registry edit <daemon_name> <--o> "['arg1', 'arg2'...]" <scheme_path> "['dep1', 'dep2'...]"`
    - `services** / **services --help`
* The service-monitor uses TOML format to communicate with CLI and GUI client
* When the service monitor attempts to read from or write to a service that is not responding it will automatically try to restart it and complete the operation.

#### Known Issues
- BaseScheme will need an additional version to support services implementing SchemeBlock, or AsyncScheme instead of Scheme (for services like disk drivers).
- If the GUI and CLI try to communicate with the service-monitor at the same time both will fail. The service monitor itself does not fail though, repeating the command should succeed.
- If the "Info" tab is clicked before selecting a service, info will be opened once a selection is made. The info tab should do nothing before a service is selected.

## Installation Guide
[click here to see the installation guide](https://gitlab.redox-os.org/CharlliePhillips/service-monitor/-/blob/main/installation-guide.md?ref_type=heads)

## Detailed Design Documentation
[click here to see the installation guide](https://gitlab.redox-os.org/CharlliePhillips/service-monitor/-/blob/main/detailed-design.pdf?ref_type=heads)

