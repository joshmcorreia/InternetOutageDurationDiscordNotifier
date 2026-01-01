use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use chrono_tz::US::Pacific;
use flexi_logger::{Duplicate, FileSpec, Logger, WriteMode};
use humantime::format_duration;
use serde::Deserialize;
use serenity::builder::ExecuteWebhook;
use serenity::http::Http;
use serenity::model::webhook::Webhook;
use std::fs;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use toml;

const GOOGLE_IP_ADDRESS: &str = "8.8.8.8";
const BOT_NAME: &str = "JoshBot";

#[cfg(windows)]
const PING_ARGS: [&str; 3] = [GOOGLE_IP_ADDRESS, "-n", "3"];
#[cfg(not(windows))]
const PING_ARGS: [&str; 3] = [GOOGLE_IP_ADDRESS, "-c", "3"];

#[derive(Deserialize, Debug)]
struct Config {
    webhook_url: String,
    poll_seconds: u64,
}

async fn internet_is_up() -> Result<bool> {
    // I'm intentionally pinging google's IP address because this tool is only
    // meant to check internet connectivity. If we ping by hostname then we're
    // also checking DNS which often goes down when doing homelab experiments :)
    let status = Command::new("ping")
        .args(PING_ARGS)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .context("Failed to ping Google")?;

    Ok(status.success())
}

async fn send_discord_message(http: &Http, webhook: &Webhook, message: &str) -> Result<()> {
    let builder = ExecuteWebhook::new().content(message).username(BOT_NAME);
    webhook
        .execute(http, false, builder)
        .await
        .context("Failed to execute Discord webhook")?;
    Ok(())
}

fn init_logger() -> Result<()> {
    Logger::try_with_str("info,serenity=warn")
        .context("Invalid logger configuration")?
        .log_to_file(FileSpec::default().directory("logs").basename("app"))
        .duplicate_to_stdout(Duplicate::All)
        .write_mode(WriteMode::BufferAndFlush)
        .format(|w, now, record| {
            write!(
                w,
                "{} [{}] {}",
                now.format("%m/%d/%Y %-I:%M:%S %p"),
                record.level(),
                record.args()
            )
        })
        .start()
        .context("Failed to start logger")?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    init_logger().context("Failed to initialize logger")?;
    log::info!("Internet Outage Duration Discord Notifier v0.1.0 started");

    let config_file = "config.toml";
    let toml_content = fs::read_to_string(config_file)
        .with_context(|| format!("Failed to read `{}`", config_file))?;
    let config: Config = toml::from_str(&toml_content)
        .with_context(|| format!("Failed to parse `{}`", config_file))?;

    anyhow::ensure!(
        config.poll_seconds >= 3,
        "Config option `poll_seconds` must be >= 3"
    );
    anyhow::ensure!(
        !config.webhook_url.trim().is_empty(),
        "Config option `webhook_url` is blank"
    );

    // Webhooks don't require a bot token
    let http = Http::new("");
    let webhook = Webhook::from_url(&http, &config.webhook_url)
        .await
        .context("Failed to create Discord webhook")?;

    let mut internet_outage_start_time: Option<DateTime<Utc>> = None;
    // We have to use a poll duration instead of interval because interval doesn't
    // currently catch Ctrl+C correctly
    let poll_duration = std::time::Duration::from_secs(config.poll_seconds);

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                log::info!("Ctrl+C received, shutting down...");
                break;
            },

            _ = tokio::time::sleep(poll_duration) => {
                match internet_is_up().await {
                    Ok(false) => {
                        if internet_outage_start_time.is_none() {
                            internet_outage_start_time = Some(Utc::now());
                            log::info!("The internet went down!");
                        }
                    }
                    Ok(true) => {
                        if let Some(start_utc) = internet_outage_start_time.take() {
                            let outage_duration = Utc::now() - start_utc;
                            let duration_formatted =
                                format_duration(Duration::from_secs(outage_duration.num_seconds() as u64))
                                    .to_string();

                            let internet_outage_message = format!(
                                "@everyone The internet went out at {} but is now back online. The outage lasted {}.",
                                start_utc.with_timezone(&Pacific).format("%m/%d/%Y %r"),
                                duration_formatted
                            );
                            log::info!("{}", internet_outage_message);

                            if let Err(err) = send_discord_message(&http, &webhook, &internet_outage_message).await {
                                log::error!("Failed to send Discord webhook: {:?}", err);
                            }
                        }
                    }
                    Err(err) => {
                        log::error!("Failed to check internet connectivity: {:?}", err);
                    }
                }
            }
        }
    }

    log::info!("Notifier stopped.");
    Ok(())
}
