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
use std::time::Duration as StdDuration;
use tokio::process::Command;
use tokio::time::Duration;
use toml;

const GOOGLE_IP_ADDRESS: &str = "8.8.8.8";
const BOT_NAME: &str = "JoshBot";

#[derive(Deserialize, Debug)]
struct Config {
    webhook_url: String,
    poll_seconds: u64,
}

async fn internet_is_up() -> Result<bool> {
    // I'm intentionally pinging google's IP address because this tool is only
    // meant to check internet connectivity. If we ping by hostname then we're
    // also checking DNS which often goes down when doing homelab experiments :)
    let mut cmd = Command::new("ping");

    #[cfg(target_os = "windows")]
    {
        cmd.args([GOOGLE_IP_ADDRESS, "-n", "3"]);
    }

    #[cfg(not(target_os = "windows"))]
    {
        cmd.args([GOOGLE_IP_ADDRESS, "-c", "3"]);
    }

    let status = cmd
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .context("Failed to ping Google")?;

    Ok(status.success())
}

async fn send_discord_message(http: &Http, message: &str, webhook_url: &str) -> Result<()> {
    let webhook = Webhook::from_url(http, webhook_url)
        .await
        .with_context(|| {
            format!(
                "Failed to create Discord webhook from URL `{}`",
                webhook_url
            )
        })?;
    let builder = ExecuteWebhook::new().content(message).username(BOT_NAME);
    webhook
        .execute(http, false, builder)
        .await
        .context("Failed to execute Discord webhook")?;
    Ok(())
}

fn init_logger() -> Result<()> {
    Logger::try_with_str("info")
        .context("Invalid logger configuration")?
        .log_to_file(FileSpec::default().directory("logs").basename("app"))
        .duplicate_to_stdout(Duplicate::All)
        .write_mode(WriteMode::BufferAndFlush)
        .format(|w, now, record| {
            write!(
                w,
                "{} [{}] {}",
                now.now().format("%m/%d/%Y %-I:%M:%S %p"),
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

    let mut internet_outage_start_time: Option<DateTime<Utc>> = None;
    let mut interval = tokio::time::interval(Duration::from_secs(config.poll_seconds));

    loop {
        interval.tick().await;

        if !internet_is_up()
            .await
            .context("Failed to check internet connectivity")?
        {
            if internet_outage_start_time.is_none() {
                internet_outage_start_time = Some(Utc::now());
                log::info!("The internet went down!");
            }
        } else if let Some(start_utc) = internet_outage_start_time.take() {
            let outage_duration = Utc::now() - start_utc;
            let duration_formatted =
                format_duration(StdDuration::from_secs(outage_duration.num_seconds() as u64))
                    .to_string();

            let internet_outage_message = format!(
                "@everyone The internet went out at {} but is now back online. The outage lasted {}.",
                start_utc.with_timezone(&Pacific).format("%m/%d/%Y %r"),
                duration_formatted
            );
            log::info!("{}", internet_outage_message);
            send_discord_message(&http, &internet_outage_message, &config.webhook_url)
                .await
                .with_context(|| {
                    format!("Failed to send Discord webhook to {}", config.webhook_url)
                })?;
        }
    }
}
