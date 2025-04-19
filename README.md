# ssh-bastion-rs

A SSH bastion client/server written in Rust.

## Building

Install [Taskfile](https://taskfile.dev/) first.

```bash
task compile-windows # or
task compile-linux
```

## Configuration

The client/server will try to find a configuration file at the following paths, tried in order:

```
"config.yml"
"config.yaml"
"/etc/ssh-bastion/config.yml"
"/etc/ssh-bastion/config.yaml"
```

### Server configuration example

```yaml
server_port: 7444
server_forward_port_start: 20000
server_forward_port_end: 24000
```

### Client configuration example

```yaml
client_server_addr: 100.2.3.101:7444
client_local_addr: 127.0.0.1:22
client_ssh_info_file: /root/ssh-info.txt
```

## Logging

Log will be written to the console as well as the following paths, tried in order:

```
"/etc/ssh-bastion/log.log"
"log.log"
```

