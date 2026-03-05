use std::path::Path;

use aws_config::BehaviorVersion;
use aws_sdk_s3::Client;
use aws_sdk_s3::config::{Credentials, Region};
use aws_sdk_s3::primitives::ByteStream;

use crate::config::AppConfig;
use crate::error::AppError;

pub async fn upload_file(config: &AppConfig, key: &str, path: &Path) -> Result<(), AppError> {
    let creds = Credentials::new(
        config.aws_access_key_id.clone(),
        config.aws_secret_access_key.clone(),
        None,
        None,
        "env",
    );

    let shared_config = aws_config::defaults(BehaviorVersion::latest())
        .region(Region::new(config.s3_region.clone()))
        .credentials_provider(creds)
        .load()
        .await;

    let s3_config = aws_sdk_s3::config::Builder::from(&shared_config)
        .endpoint_url(config.s3_endpoint.clone())
        .force_path_style(true)
        .build();

    let client = Client::from_conf(s3_config);
    let body = ByteStream::from_path(path)
        .await
        .map_err(|e| AppError::S3Upload(e.to_string()))?;

    client
        .put_object()
        .bucket(&config.s3_bucket)
        .key(key)
        .content_type("video/mp4")
        .body(body)
        .send()
        .await
        .map_err(|e| AppError::S3Upload(e.to_string()))?;

    Ok(())
}
