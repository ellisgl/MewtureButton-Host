# Mewture Button Host Software

Speaks to a microcontroller using the DDAA (Ding Ding Ack Ack) protocol.
This allows an external device to set the mute status (Pulse Audio for Linux at the moment),
reporting changes back to the microcontroller, so it can display it.
Currently, it's setup to use Systemd to control the service. 

See <https://github.com/ellisgl/MewtureButton-Firmware>

## Install from release:
Download the `mewture_button_#.#.#_amd64.deb`

```shell
sudo dpkg -i mewture_button_#.#.#_amd64.deb
mewture_setup
systemctl --user start mewture_daemon
```

## Compile and install:

```shell
cargo build -r && cargo deb -p mewture_daemon
sudo dpkg -i target/debian/mewture_button_#.#.#_amd64.deb
mewture_setup
systemctl --user start mewture_daemon
```
