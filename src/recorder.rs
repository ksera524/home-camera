use std::path::Path;
use std::process::Command;
use std::time::Duration;

use shiguredo_http11::uri::Uri;

use crate::error::AppError;

#[derive(Debug, Clone, Copy)]
pub struct RecorderStats {
    pub recorded_seconds: u64,
}

pub fn record_to_mp4(
    rtsp_url: &str,
    output_path: &Path,
    duration: Duration,
    ffmpeg_preset: &str,
    ffmpeg_crf: u8,
) -> Result<RecorderStats, AppError> {
    validate_rtsp_url(rtsp_url)?;

    let duration_secs = duration.as_secs().max(1);
    let output = Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("warning")
        .arg("-rtsp_transport")
        .arg("tcp")
        .arg("-i")
        .arg(rtsp_url)
        .arg("-t")
        .arg(duration_secs.to_string())
        .arg("-an")
        .arg("-c:v")
        .arg("libx264")
        .arg("-preset")
        .arg(ffmpeg_preset)
        .arg("-crf")
        .arg(ffmpeg_crf.to_string())
        .arg("-pix_fmt")
        .arg("yuv420p")
        .arg("-movflags")
        .arg("+faststart")
        .arg("-y")
        .arg(output_path)
        .output()
        .map_err(|e| AppError::Recording(format!("failed to start ffmpeg: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let reason = stderr.lines().last().unwrap_or("unknown ffmpeg error");
        return Err(AppError::Recording(format!(
            "ffmpeg failed with status {}: {reason}",
            output.status
        )));
    }

    Ok(RecorderStats {
        recorded_seconds: duration_secs,
    })
}

fn validate_rtsp_url(rtsp_url: &str) -> Result<(), AppError> {
    let uri = Uri::parse(rtsp_url).map_err(|e| AppError::Uri(e.to_string()))?;
    let scheme = uri.scheme().unwrap_or_default().to_ascii_lowercase();
    if scheme != "rtsp" {
        return Err(AppError::Recording(
            "RTSP_URL scheme must be rtsp".to_string(),
        ));
    }
    if uri.host().is_none() {
        return Err(AppError::Recording(
            "RTSP_URL must include host".to_string(),
        ));
    }
    Ok(())
}
