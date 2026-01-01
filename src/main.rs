use chrono::{DateTime, TimeDelta, Utc};
use chrono_tz::Tz;
use chrono_tz::US::Pacific;
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

#[derive(Deserialize, Debug)]
struct Config {
    webhook_url: String,
    poll_seconds: u64,
}

fn format_timedelta_hhmmss(delta: TimeDelta) -> String {
    let total_seconds = delta.num_seconds();
    let mut formatted_string: String = "".to_string();
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    if hours > 0 {
        let hours_string = format!("{} hours ", hours);
        formatted_string += &hours_string;
    }
    if minutes > 0 {
        let minutes_string = format!("{} minutes ", minutes);
        formatted_string += &minutes_string;
    }
    if seconds > 0 {
        let seconds_string = format!("{} seconds ", seconds);
        formatted_string += &seconds_string;
    }
    // remove the trailing space
    formatted_string.pop();
    return formatted_string;
}

async fn send_discord_message(
    http: &Http,
    message: &str,
    webhook_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let webhook = Webhook::from_url(http, webhook_url).await?;
    let builder = ExecuteWebhook::new()
        .content(message)
        .username("JoshBot");

    webhook.execute(http, false, builder).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let script_start_time = Utc::now().with_timezone(&Pacific);
    println!(
        "Internet Outage Duration Discord Notifier 0.1.0 initialized on {}.",
        script_start_time.format("%m/%d/%Y %r")
    );

    let toml_content = fs::read_to_string("config.toml")?;
    let config: Config = toml::from_str(&toml_content)?;

    let mut internet_outage_start_time: Option<DateTime<Tz>> = None;

    let http = Http::new("");

    loop {
        // I'm intentionally pinging google's IP address because this tool is only
        // meant to check internet connectivity. If we ping by hostname then we're
        // also checking DNS which often goes down when doing homelab experiments :)
        let ping_google_ip_result = Command::new("ping")
            .arg(GOOGLE_IP_ADDRESS)
            .arg("-c")
            // check 3 times to ensure that it wasn't just a one-time fluke
            .arg("3")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await?;
        if !ping_google_ip_result.success() {
            // only set the internet_outage_start_time when the internet first goes out, otherwise
            // the time will be continuously updated even though it's the same outage
            if internet_outage_start_time.is_none() {
                internet_outage_start_time = Some(Utc::now().with_timezone(&Pacific));
                println!(
                    "The internet went down at {}!",
                    internet_outage_start_time.unwrap().format("%m/%d/%Y %r")
                );
            }
            sleep(Duration::from_secs(config.poll_seconds)).await;
            continue;
        }
        if internet_outage_start_time.is_some() {
            let outage_duration =
                Utc::now().with_timezone(&Pacific) - internet_outage_start_time.unwrap();
            let outage_duration_hhmmss = format_timedelta_hhmmss(outage_duration);
            let internet_outage_message = format!(
                "@everyone The internet went out at {} but is now back online. The outage lasted {}.",
                internet_outage_start_time.unwrap().format("%m/%d/%Y %r"),
                outage_duration_hhmmss
            );
            send_discord_message(&http, &internet_outage_message, &config.webhook_url).await?;
            internet_outage_start_time = None;
        }

        sleep(Duration::from_secs(config.poll_seconds)).await;
    }
}
