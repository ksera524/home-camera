use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("missing required environment variable: {0}")]
    MissingEnv(&'static str),

    #[error("invalid environment variable {name}: {reason}")]
    InvalidEnv { name: &'static str, reason: String },

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("time error: {0}")]
    Time(#[from] time::error::ComponentRange),

    #[error("rtsp error: {0}")]
    Rtsp(#[from] shiguredo_rtsp::Error),

    #[error("uri parse error: {0}")]
    Uri(String),

    #[error("sdp parse error: {0}")]
    Sdp(String),

    #[error("mp4 mux error: {0}")]
    Mp4Mux(String),

    #[error("s3 upload error: {0}")]
    S3Upload(String),

    #[error("recording failed: {0}")]
    Recording(String),
}
