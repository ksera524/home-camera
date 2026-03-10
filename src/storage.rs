use std::fs::File;
use std::io::Read;
use std::path::Path;

use reqwest::{Client, Method};
use shiguredo_s3::api::{
    CreateMultipartUploadFluentBuilder, S3Request, S3Response, UploadPartFluentBuilder,
};
use shiguredo_s3::types::{CompletedMultipartUpload, CompletedPart};
use shiguredo_s3::{Credential, S3Client, S3Config};

use crate::config::AppConfig;
use crate::error::AppError;

const MULTIPART_PART_SIZE: usize = 8 * 1024 * 1024;

pub async fn upload_file(config: &AppConfig, key: &str, path: &Path) -> Result<(), AppError> {
    let s3_config = S3Config::builder()
        .region(config.s3_region.clone())
        .credential(Credential::new(
            config.aws_access_key_id.clone(),
            config.aws_secret_access_key.clone(),
        ))
        .endpoint(config.s3_endpoint.clone())
        .use_path_style(true)
        .build()
        .map_err(|e| AppError::S3Upload(e.to_string()))?;
    let client = S3Client::new(s3_config);
    let http = Client::builder()
        .build()
        .map_err(|e| AppError::S3Upload(e.to_string()))?;

    let mut file = File::open(path).map_err(|e| AppError::S3Upload(e.to_string()))?;

    let create_request = client
        .create_multipart_upload()
        .bucket(&config.s3_bucket)
        .key(key)
        .content_type("video/mp4")
        .build_request()
        .map_err(|e| AppError::S3Upload(e.to_string()))?;
    let create_response = execute_s3_request(&http, create_request).await?;
    let create_output = CreateMultipartUploadFluentBuilder::parse_response(&create_response)
        .map_err(|e| AppError::S3Upload(e.to_string()))?;
    let upload_id = create_output
        .upload_id
        .ok_or_else(|| AppError::S3Upload("missing upload_id in multipart response".to_string()))?;

    let mut completed_parts = Vec::new();
    let mut part_number: i32 = 1;
    let mut buffer = vec![0_u8; MULTIPART_PART_SIZE];

    loop {
        let read_bytes = file
            .read(&mut buffer)
            .map_err(|e| AppError::S3Upload(e.to_string()))?;
        if read_bytes == 0 {
            break;
        }

        let upload_request = client
            .upload_part()
            .bucket(&config.s3_bucket)
            .key(key)
            .upload_id(&upload_id)
            .part_number(part_number)
            .body(buffer[..read_bytes].to_vec())
            .build_request()
            .map_err(|e| AppError::S3Upload(e.to_string()))?;
        let upload_response = execute_s3_request(&http, upload_request).await?;
        let upload_output = UploadPartFluentBuilder::parse_response(&upload_response)
            .map_err(|e| AppError::S3Upload(e.to_string()))?;

        completed_parts.push(CompletedPart {
            e_tag: upload_output.e_tag,
            part_number: Some(part_number),
        });
        part_number += 1;
    }

    if completed_parts.is_empty() {
        return Err(AppError::S3Upload(
            "multipart upload produced no parts".to_string(),
        ));
    }

    let complete_request = client
        .complete_multipart_upload()
        .bucket(&config.s3_bucket)
        .key(key)
        .upload_id(&upload_id)
        .multipart_upload(CompletedMultipartUpload {
            parts: Some(completed_parts),
        })
        .build_request()
        .map_err(|e| AppError::S3Upload(e.to_string()))?;
    let complete_response = execute_s3_request(&http, complete_request).await?;
    shiguredo_s3::api::CompleteMultipartUploadFluentBuilder::parse_response(&complete_response)
        .map_err(|e| AppError::S3Upload(e.to_string()))?;

    Ok(())
}

async fn execute_s3_request(http: &Client, s3_request: S3Request) -> Result<S3Response, AppError> {
    let method = Method::from_bytes(s3_request.method.as_bytes())
        .map_err(|e| AppError::S3Upload(e.to_string()))?;
    let scheme = if s3_request.https { "https" } else { "http" };
    let url = format!(
        "{scheme}://{}:{}{}",
        s3_request.host, s3_request.port, s3_request.uri
    );

    let mut request_builder = http.request(method, url);
    for (name, value) in &s3_request.headers {
        request_builder = request_builder.header(name, value);
    }

    if !s3_request.body.is_empty() {
        request_builder = request_builder.body(s3_request.body);
    }

    let response = request_builder
        .send()
        .await
        .map_err(|e| AppError::S3Upload(e.to_string()))?;
    let status_code = response.status().as_u16();
    let headers = response
        .headers()
        .iter()
        .map(|(name, value)| {
            let value = value.to_str().unwrap_or("").to_string();
            (name.to_string(), value)
        })
        .collect();
    let body = response
        .bytes()
        .await
        .map_err(|e| AppError::S3Upload(e.to_string()))?
        .to_vec();

    Ok(S3Response {
        status_code,
        headers,
        body,
    })
}
