use anyhow::{Context, Result};
use chrono::{DateTime, TimeDelta, Utc};
use chrono_tz::US::Pacific;
use flexi_logger::{Duplicate, FileSpec, Logger, WriteMode};
use serde::Deserialize;
use serenity::builder::ExecuteWebhook;
use serenity::http::Http;
use serenity::model::webhook::Webhook;
use std::fs;
use std::process::Stdio;
use tokio::process::Command;
use tokio::time::{Duration, sleep};
use toml;

const GOOGLE_IP_ADDRESS: &str = "8.8.8.8";
const BOT_NAME: &str = "JoshBot";

#[derive(Deserialize, Debug)]
struct Config {
    webhook_url: String,
    poll_seconds: u64,
}

fn format_timedelta_hhmmss(delta: TimeDelta) -> String {
    let total_seconds = delta.num_seconds().max(0);

    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    let mut parts = Vec::new();

    if hours > 0 {
        parts.push(format!("{} hours", hours));
    }
    if minutes > 0 {
        parts.push(format!("{} minutes", minutes));
    }
    if seconds > 0 || parts.is_empty() {
        parts.push(format!("{} seconds", seconds));
    }

    parts.join(" ")
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
        .await?;

    Ok(status.success())
}

async fn send_discord_message(http: &Http, message: &str, webhook_url: &str) -> Result<()> {
    let webhook = Webhook::from_url(http, webhook_url).await?;
    let builder = ExecuteWebhook::new().content(message).username(BOT_NAME);
    webhook.execute(http, false, builder).await?;
    Ok(())
}

fn init_logger() -> Result<()> {
    Logger::try_with_str("info")?
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
        .start()?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    init_logger().context("Failed to initialize logger")?;
    log::info!("Internet Outage Duration Discord Notifier v0.1.0 started");

    let config_file = "config.toml";
    let toml_content =
        fs::read_to_string(config_file).context(format!("Failed to read {}", config_file))?;
    let config: Config =
        toml::from_str(&toml_content).context(format!("Failed to parse {}", config_file))?;

    if config.poll_seconds < 3 {
        anyhow::bail!("Config option poll_seconds must be >= 3");
    }

    let mut internet_outage_start_time: Option<DateTime<Utc>> = None;
    // Webhooks don't require a bot token
    let http = Http::new("");

    loop {
        if !internet_is_up().await? {
            if internet_outage_start_time.is_none() {
                let start_utc = Utc::now();
                internet_outage_start_time = Some(start_utc);
                log::info!("The internet went down!");
            }
        } else if let Some(start_utc) = internet_outage_start_time {
            let outage_duration = Utc::now() - start_utc;
            let outage_duration_hhmmss = format_timedelta_hhmmss(outage_duration);
            let internet_outage_message = format!(
                "@everyone The internet went out at {} but is now back online. The outage lasted {}.",
                start_utc.with_timezone(&Pacific).format("%m/%d/%Y %r"),
                outage_duration_hhmmss
            );
            log::info!("{}", internet_outage_message);
            send_discord_message(&http, &internet_outage_message, &config.webhook_url)
                .await
                .context("Failed to send Discord webhook")?;
            internet_outage_start_time = None;
        }

        sleep(Duration::from_secs(config.poll_seconds)).await;
    }
}
