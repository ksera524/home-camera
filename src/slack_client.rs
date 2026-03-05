use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::{Value, json};

const DEFAULT_SLACK_BASE_URL: &str = "http://slack:3000";

pub async fn post_message(http: &Client, channel: &str, text: &str) -> Result<(u16, String)> {
    post_message_to(http, DEFAULT_SLACK_BASE_URL, channel, text).await
}

pub async fn post_message_to(
    http: &Client,
    base_url: &str,
    channel: &str,
    text: &str,
) -> Result<(u16, String)> {
    let data = json!({
        "channel": channel,
        "text": text
    });
    post_json(
        http,
        &format!("{}/slack/message", normalize_base_url(base_url)),
        data,
    )
    .await
}

async fn post_json(http: &Client, url: &str, payload: Value) -> Result<(u16, String)> {
    let response = http
        .post(url)
        .json(&payload)
        .send()
        .await
        .with_context(|| format!("failed to post to {url}"))?;

    let status = response.status().as_u16();
    let body = response.text().await.unwrap_or_default();
    Ok((status, body))
}

fn normalize_base_url(base_url: &str) -> &str {
    base_url.trim_end_matches('/')
}

#[cfg(test)]
mod tests {
    use super::normalize_base_url;
    use proptest::prelude::*;

    #[test]
    fn normalize_base_url_trims_trailing_slashes() {
        assert_eq!(
            normalize_base_url("http://slack:3000/"),
            "http://slack:3000"
        );
    }

    proptest! {
        #[test]
        fn pbt_normalize_base_url_is_idempotent(host in "[a-z]{3,10}", count in 0usize..5) {
            let suffix = "/".repeat(count);
            let input = format!("http://{host}:3000{suffix}");
            let once = normalize_base_url(&input).to_string();
            let twice = normalize_base_url(&once).to_string();
            prop_assert_eq!(once, twice);
        }
    }
}
