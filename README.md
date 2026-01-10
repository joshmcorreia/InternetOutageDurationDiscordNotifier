# Internet Outage Notifier
A simple tool for getting notifications when you internet goes out. Sends a ntfy message to your local server as soon as the internet goes out, and sends a discord message to your server once the internet is restored. Useful for tracking internet outages over time and determining if your ISP has a common outage time. Intended for usage on an OpenWRT router, but not optimized for size.

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
$ scp config.toml target/aarch64-unknown-linux-musl/release/internet_outage_notifier ROUTER:/root
```

# Example config
```
discord_webhook_url = "https://discord.com/api/webhooks/12345/abdef"
ntfy_url = "https://ntfy.myserver.com/outages"
enable_ntfy = true
poll_seconds = 5
```
