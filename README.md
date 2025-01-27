# service-monitor

1. `git clone` into recipes folder
2. add the line `service-monitor = {}` to `[packages]` in `config/desktop.toml`
3. add `service-monitor_service-monitor` as the last line in `cookbook/recipes/core/initfs/init.rc` 
4. `make r.service-monitor cr.initfs desktop`




# Testing

1. launch Redox in VM and open terminal
2. `service-monitor_services` starts the cli tool
3. `service-monitor_services stop service-monitor_gtrand` will stop gtrand
4. `service-monitor_services start service-monitor_gtrand` will start gtrand
5. use `ps` to see running processes and `cat /scheme/sys/log` to view the log