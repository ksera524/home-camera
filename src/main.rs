use std::path::PathBuf;
use std::time::Duration;

use home_camera::config::AppConfig;
use home_camera::error::AppError;
use home_camera::key::build_object_key;
use home_camera::recorder::record_to_mp4;
use home_camera::storage::upload_file;
use time::{OffsetDateTime, UtcOffset};

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), AppError> {
    let config = AppConfig::from_env()?;
    let now = OffsetDateTime::now_utc().to_offset(UtcOffset::UTC);
    let object_key = build_object_key(&config.camera_id, now);

    let temp_file = temp_mp4_path(&config.camera_id, now.unix_timestamp());
    println!("recording started: {}", config.rtsp_url);
    println!("temp file: {}", temp_file.display());

    let stats = record_to_mp4(
        &config.rtsp_url,
        &temp_file,
        Duration::from_secs(config.record_seconds),
    )?;

    println!(
        "recording completed: rtp_packets={}, access_units={}",
        stats.rtp_packets, stats.access_units
    );
    println!("uploading to s3://{}/{}", config.s3_bucket, object_key);

    upload_file(&config, &object_key, &temp_file).await?;
    println!("upload completed");

    let _ = std::fs::remove_file(&temp_file);
    Ok(())
}

fn temp_mp4_path(camera_id: &str, unix_ts: i64) -> PathBuf {
    let safe_id: String = camera_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    PathBuf::from(format!("/tmp/{safe_id}-{unix_ts}.mp4"))
}
