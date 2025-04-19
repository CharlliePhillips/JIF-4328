# Service Monitor Installation Guide
## Prerequisites:
- A working build of Redox OS
  - Check you are using a compatible OS to build with and that you have the prerequisites listed here: https://doc.redox-os.org/book/advanced-podman-build.html#installation
  - Build instructions can be found here: https://doc.redox-os.org/book/podman-build.html#tldr---new-or-existing-working-directory
  - QEMU

## Dependencies
- No separate dependencies outside of the ones built into Redox. As long as your build is up to date, it will have all dependencies needed.

## Download and Build Instructions
- Once you have a working build of Redox, navigate to the recipes folder `redox/cookbook/recipes` and clone the service-monitor (https://gitlab.redox-os.org/CharlliePhillips/service-monitor.git) into that folder.
- Add the line `service-monitor = {}` under `[packages]` in `desktop.toml` (`redox/config/desktop.toml`)
- Add `service-monitor` as the last line in init.rc (`redox/cookbook/recipes/core/initfs/init.rc`)
- Run `make r.service-monitor cr.initfs desktop` in the terminal from the `redox` main folder.
- Run `make qemu` to launch the created image in a virtual machine

## Running the Service Monitor
- The service monitor will automatically run on boot, and will start all services in `registry.toml` afterwards.
  - `ps` can be run in the command line to view all running processes.
- The commands for the service-monitor can be found by running `services --help` in the command line.
- `services-gui` can also be run to view the GUI version of the service monitor.

## Troubleshooting
### Issues Building Redox
- Redox can be tricky to get built as the OS is constantly in development.
- For general build troubleshooting, it is worth checking here for a guide: https://doc.redox-os.org/book/troubleshooting.html#troubleshooting-the-build
- For more specific help, ask in the Redox Matrix Space: https://matrix.to/#/#redox:matrix.org

### Service Monitor Issues
- If you are running into issues compiling with the service monitor, check that:
  - You cloned it into the right folder
  - You have edited the files above (`desktop.toml`, `init.fs`) with their respective additions correctly
- If all of the above is correct, a fresh build of Redox could help resolve the issue (although this will take a while)
- If you are still having issues, read through the error message in case it has a quick fix, or stop by the Matrix Space for further assistance.