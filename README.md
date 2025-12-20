# Internet Outage Duration Discord Notifier
A simple tool for getting a discord message after an internet outage. Useful for tracking internet outages over time and determining if your ISP has a common outage time. Intended for usage on an OpenWRT router, but not optimized for size.

Populate `config.toml` before running the program.

# Build Pre-requisites
[cross-rs](https://github.com/cross-rs/cross)
```
$ cargo install cross --git https://github.com/cross-rs/cross
```

# Building the binary
```
$ make build
```

# Copying the binary to your router
```
$ scp target/aarch64-unknown-linux-musl/release/internet_outage_duration_discord_notifier ROUTER:/root
```
