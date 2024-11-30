use std::{future::ready, str::FromStr, sync::Arc};

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

use super::{
    helpers::{FutureAnyhowExt, ReqwestPrettyJsonResponse},
    DownloadTask,
};

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
#[serde(transparent)]
pub struct DownloadLinkResponse(Vec<NexusDownloadLink>);

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
        fn header<E, T>(headers: &HeaderMap, header_name: &str) -> Result<T>
        where
            E: Send + Sync + std::fmt::Display + std::fmt::Debug + 'static,
            T: FromStr<Err = E> + Send + Sync + 'static,
        {
            headers
                .get(header_name)
                .context("no header")
                .and_then(|value| value.to_str().context("header is not a string"))
                .and_then(|value| {
                    value
                        .parse::<T>()
                        .map_err(|message| anyhow::anyhow!("{message:?}"))
                        .context("invalid type")
                })
                .with_context(|| format!("extracting [{header_name}] header"))
        }

        Ok(Self {
            hourly_limit: header(headers, "X-RL-Hourly-Limit")?,
            hourly_remaining: header(headers, "X-RL-Hourly-Remaining")?,
            hourly_reset: header(headers, "X-RL-Hourly-Reset")?,
            daily_limit: header(headers, "X-RL-Daily-Limit")?,
            daily_remaining: header(headers, "X-RL-Daily-Remaining")?,
            daily_reset: header(headers, "X-RL-Daily-Reset")?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NexusDownloadLink {
    #[serde(rename = "URI")]
    pub uri: url::Url,
    pub name: String,
    pub short_name: String,
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
                "{BASE_URL}/v1/games/{game_domain_name}/mods/{mod_id}/files/{file_id}/download_link.json"
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
    pub async fn download(self: Arc<Self>, request: DownloadFileRequest) -> Result<url::Url> {
        self.clone()
            .generate_download_link(&request)
            .and_then(|download_link| {
                download_link
                    .0
                    .into_iter()
                    .next()
                    .context("no preferred download link found")
                    .pipe(ready)
            })
            .map_ok(|link| link.uri)
            .await
    }
}
