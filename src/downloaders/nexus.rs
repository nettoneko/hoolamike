use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use futures::{FutureExt, TryFutureExt};
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Client, ClientBuilder, Response,
};
use serde::{Deserialize, Serialize};
use tap::prelude::*;

use crate::config_file::NexusConfig;

use super::helpers::{FutureAnyhowExt, ReqwestPrettyJsonResponse};

pub struct NexusDownloader {
    client: Client,
}

const AUTH_HEADER: &str = "apikey";
const BASE_URL: &str = "https://api.nexusmods.com";

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct DownloadFileRequest {
    pub game_domain_name: String,
    pub mod_id: usize,
    pub file_id: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadLinkResponse {
    #[serde(flatten)]
    pub data: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug)]
pub struct ThrottlingHeaders {
    /// X-RL-Hourly-Limit →100
    pub hourly_limit: usize,
    /// X-RL-Hourly-Remaining →96
    pub hourly_remaining: usize,
    /// X-RL-Hourly-Reset →2019-02-01T12:00:00+00:00
    pub hourly_reset: DateTime<Utc>,
    /// X-RL-Daily-Limit →2500
    pub daily_limit: usize,
    /// X-RL-Daily-Remaining →2488
    pub daily_remaining: usize,
    /// X-RL-Daily-Reset →2019-02-02 00:00:00 +0000
    pub daily_reset: DateTime<Utc>,
}

impl ThrottlingHeaders {
    fn from_response(response: &Response) -> Result<Self> {
        let headers = response.headers();
        Ok(Self {
            hourly_limit: headers
                .get("X-RL-Hourly-Limit")
                .context("no header")
                .and_then(|value| value.to_str().context("header is not a string"))
                .and_then(|value| value.parse().context("invalid type"))
                .context("extracting [X-RL-Hourly-Limit] header")?,
            hourly_remaining: headers
                .get("X-RL-Hourly-Remaining")
                .context("no header")
                .and_then(|value| value.to_str().context("header is not a string"))
                .and_then(|value| value.parse().context("invalid type"))
                .context("extracting [X-RL-Hourly-Remaining] header")?,
            hourly_reset: headers
                .get("X-RL-Hourly-Reset")
                .context("no header")
                .and_then(|value| value.to_str().context("header is not a string"))
                .and_then(|value| value.parse().context("invalid type"))
                .context("extracting [X-RL-Hourly-Reset] header")?,
            daily_limit: headers
                .get("X-RL-Daily-Limit")
                .context("no header")
                .and_then(|value| value.to_str().context("header is not a string"))
                .and_then(|value| value.parse().context("invalid type"))
                .context("extracting [X-RL-Daily-Limit] header")?,
            daily_remaining: headers
                .get("X-RL-Daily-Remaining")
                .context("no header")
                .and_then(|value| value.to_str().context("header is not a string"))
                .and_then(|value| value.parse().context("invalid type"))
                .context("extracting [X-RL-Daily-Remaining] header")?,
            daily_reset: headers
                .get("X-RL-Daily-Reset")
                .context("no header")
                .and_then(|value| value.to_str().context("header is not a string"))
                .and_then(|value| value.parse().context("invalid type"))
                .context("extracting [X-RL-Daily-Reset] header")?,
        })
    }
}

impl NexusDownloader {
    pub fn new(api_key: String) -> Result<Self> {
        [(AUTH_HEADER, api_key)]
            .into_iter()
            .map(|(key, value)| {
                HeaderValue::from_str(&value)
                    .with_context(|| format!("invalid header value for {key}"))
                    .map(|value| (key, value))
            })
            .try_fold(HeaderMap::new(), |map, header| {
                header.map(|(key, value)| map.tap_mut(|map| map.insert(key, value).pipe(|_| ())))
            })
            .and_then(|headers| {
                ClientBuilder::new()
                    .default_headers(headers)
                    .build()
                    .context("building http client")
            })
            .map(|client| Self { client })
            .context("building NexusDownloader")
    }

    async fn generate_download_link(
        self: Arc<Self>,
        DownloadFileRequest {
            game_domain_name,
            mod_id,
            file_id,
        }: &DownloadFileRequest,
    ) -> Result<DownloadLinkResponse> {
        self.client
            .get(format!(
                "{BASE_URL}/v1/{game_domain_name}/mods/{mod_id}/files/{file_id}/download_link.json"
            ))
            .send()
            .map_context("sending request")
            .inspect_ok(|response| {
                ThrottlingHeaders::from_response(response)
                    .tap_ok(|response| tracing::info!("{response:?}"))
                    .pipe(|_| ())
            })
            .and_then(|response| response.json_response_ok(|_| Ok(())))
            .await
    }
    pub async fn download(self: Arc<Self>, request: DownloadFileRequest) -> Result<()> {
        self.clone()
            .generate_download_link(&request)
            .map_ok(|download_link| panic!("could not parse {download_link:?}"))
            .await
    }
}
