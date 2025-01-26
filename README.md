# service-monitor

1. clone into recipes folder
2. make r.service-monitor
3. add service-monitor to packages in config/desktop.toml
4. add service-monitor as last line to in recipes/core/initfs/init.rc 
5. make rebuild cr.initfs desktop

# Testing
1. launch Redox in VM and open terminal
2. 'service-monitor_services' starts the cli tool
3. 'service-monitor_services stop gtrand' will stop gtrand
4. 'service-monitor_services start gtrand' will start gtrand
5. use 'ps' to see running processes and 'cat /scheme/sys/log' to view the log 