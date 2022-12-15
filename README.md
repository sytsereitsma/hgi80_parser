 Install libudev-dev:
 ```
 sudo apt get libudev-dev
 ```

 ## Add as system service

```sh
sudo vim /etc/systemd/system/evohome.service
```

```toml
[Unit]
Description=Evohome HGI80 temperature logger
After=network.target

[Service]
User=sytse
ExecStart=hgi80-decoder --usb /dev/ttyUSB0 --endpoint http://sdfsdfsdf

[Install]
WantedBy=multi-user.target
```

```sh
sudo systemctl enable evohome
sudo systemctl start evohome
```