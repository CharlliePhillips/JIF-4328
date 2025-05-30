# Service Monitor
A system health monitoring service for Redox OS

## Release Notes

### Version 1.0.0
#### Features
* Help option in command line, add `-h` or `--help` to any command for usage information
* The service-monitor can start daemons and manage those that use the `BaseScheme` API.
* The service monitor starts at boot and starts its registered services.
`services start gtrand` and `services stop gtrand`
* The commands `start` and `stop` can be used in the command line to manually start and stop registered services.
`services-gui`
* GUI is available for quick access to a list of services, statistics, and controls.
`services list`
* The command `list` can be used to see a list of all registered services and some relevant data.
`services info gtrand`
* The command `info` can be used to retrieve detailed data on a particular service.
`services clear gtrand`
* The command `clear` can be used to clear the short term data stored in a service.
* The registry commands `services registry ...` can be used to view and edit the registry.
    - `services registry view <daemon_name>`
    - `services registry add <--old> <daemon_name> "['arg1', 'arg2'...]" <--override> "['dep1', 'dep2'...]" <scheme_path>`
    - `services registry remove <daemon_name>`
    - `services registry edit <daemon_name> <--o> "['arg1', 'arg2'...]" <scheme_path> "['dep1', 'dep2'...]"`
    - `services** / **services --help`
* The service-monitor uses TOML format to communicate with CLI and GUI client.
* When the service monitor attempts to read from or write to a service that is not responding, it will automatically try to restart it and complete the operation.

#### Known Issues
- BaseScheme will need an additional version to support services implementing SchemeBlock, or AsyncScheme instead of Scheme (for services like disk drivers).
- If the GUI and CLI try to communicate with the service-monitor at the same time both will fail. The service monitor itself does not fail though, repeating the command should succeed.
- If the "Info" tab is clicked before selecting a service, info will be opened once a selection is made. The info tab should do nothing before a service is selected.

#### Squashed Bugs
- The info and list commands now properly display services that are not running.
- When attempting to run the service recovery test too quickly the whole OS will freeze. This is likely due to the threading used for timeout detection, other components may need to be refactored for multithreading for this to be fixed.
- Excluding the dependencies argument from services registry edit causes a panic
- Error text for depends list for services registry add prefixes the list with 'args' instead of 'depends'
- Depends list for services registry add is shown as optional, but omitting causes clap parser to demand it
- Fixed 2 regressions with services clear and the BaseScheme API
- Fixed timeout recovery test (gtrand2 times out when attempting to start it twice).
- GUI table would clear current selection upon refresh.
- GUI table would have any sorting reset upon refresh.

## Installation Guide
[click here to see the installation guide](https://github.com/CharlliePhillips/JIF-4328/blob/main/installation-guide.md)

## Detailed Design Documentation
[click here to see the detailed design docs](https://github.com/CharlliePhillips/JIF-4328/blob/main/detailed-design.pdf)

