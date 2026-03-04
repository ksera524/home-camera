use std::collections::HashMap;

use crate::error::AppError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    pub rtsp_url: String,
    pub s3_endpoint: String,
    pub s3_region: String,
    pub aws_access_key_id: String,
    pub aws_secret_access_key: String,
    pub s3_bucket: String,
    pub camera_id: String,
    pub record_seconds: u64,
    pub ffmpeg_preset: String,
    pub ffmpeg_crf: u8,
}

impl AppConfig {
    pub const DEFAULT_BUCKET: &'static str = "home-camera-recordings";
    pub const DEFAULT_CAMERA_ID: &'static str = "camera";
    pub const DEFAULT_RECORD_SECONDS: u64 = 3600;
    pub const DEFAULT_FFMPEG_PRESET: &'static str = "veryfast";
    pub const DEFAULT_FFMPEG_CRF: u8 = 23;

    pub fn from_env() -> Result<Self, AppError> {
        let mut map = HashMap::new();
        for (k, v) in std::env::vars() {
            map.insert(k, v);
        }
        Self::from_map(&map)
    }

    pub fn from_map(vars: &HashMap<String, String>) -> Result<Self, AppError> {
        let rtsp_url = required(vars, "RTSP_URL")?;
        if !rtsp_url.starts_with("rtsp://") {
            return Err(AppError::InvalidEnv {
                name: "RTSP_URL",
                reason: "must start with rtsp://".to_string(),
            });
        }

        let s3_endpoint = required(vars, "RUSTFS_S3_ENDPOINT")?;
        let s3_region = required(vars, "RUSTFS_S3_REGION")?;
        let aws_access_key_id = required(vars, "AWS_ACCESS_KEY_ID")?;
        let aws_secret_access_key = required(vars, "AWS_SECRET_ACCESS_KEY")?;
        let s3_bucket = vars
            .get("S3_BUCKET")
            .cloned()
            .unwrap_or_else(|| Self::DEFAULT_BUCKET.to_string());
        let camera_id = vars
            .get("CAMERA_ID")
            .cloned()
            .unwrap_or_else(|| Self::DEFAULT_CAMERA_ID.to_string());

        let record_seconds = match vars.get("RECORD_SECONDS") {
            Some(v) => {
                let parsed = v.parse::<u64>().map_err(|_| AppError::InvalidEnv {
                    name: "RECORD_SECONDS",
                    reason: "must be an integer".to_string(),
                })?;
                if parsed == 0 {
                    return Err(AppError::InvalidEnv {
                        name: "RECORD_SECONDS",
                        reason: "must be > 0".to_string(),
                    });
                }
                parsed
            }
            None => Self::DEFAULT_RECORD_SECONDS,
        };

        let ffmpeg_preset = vars
            .get("FFMPEG_PRESET")
            .cloned()
            .unwrap_or_else(|| Self::DEFAULT_FFMPEG_PRESET.to_string());
        if ffmpeg_preset.trim().is_empty() {
            return Err(AppError::InvalidEnv {
                name: "FFMPEG_PRESET",
                reason: "must not be empty".to_string(),
            });
        }

        let ffmpeg_crf = match vars.get("FFMPEG_CRF") {
            Some(v) => {
                let parsed = v.parse::<u8>().map_err(|_| AppError::InvalidEnv {
                    name: "FFMPEG_CRF",
                    reason: "must be an integer".to_string(),
                })?;
                if parsed > 51 {
                    return Err(AppError::InvalidEnv {
                        name: "FFMPEG_CRF",
                        reason: "must be <= 51".to_string(),
                    });
                }
                parsed
            }
            None => Self::DEFAULT_FFMPEG_CRF,
        };

        Ok(Self {
            rtsp_url,
            s3_endpoint,
            s3_region,
            aws_access_key_id,
            aws_secret_access_key,
            s3_bucket,
            camera_id,
            record_seconds,
            ffmpeg_preset,
            ffmpeg_crf,
        })
    }
}

fn required(vars: &HashMap<String, String>, key: &'static str) -> Result<String, AppError> {
    vars.get(key).cloned().ok_or(AppError::MissingEnv(key))
}
