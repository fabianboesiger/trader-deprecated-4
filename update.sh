#!/bin/bash
sudo systemctl stop trader
git pull
cargo build --release
sudo systemctl start trader
sudo journalctl -f -u trader