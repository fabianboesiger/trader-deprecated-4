#!/bin/bash
git pull
cargo build --release --features=live
sudo systemctl restart trader
sudo journalctl -f -u trader