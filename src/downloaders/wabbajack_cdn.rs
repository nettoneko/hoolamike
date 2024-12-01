use {
    super::helpers::FutureAnyhowExt,
    crate::modlist_json::WabbajackCDNDownloaderState,
    anyhow::{Context, Result},
    flate2::read::GzDecoder,
    futures::{StreamExt, TryFutureExt},
    itertools::Itertools,
    reqwest::Client,
    serde::{Deserialize, Serialize},
    std::{
        future::ready,
        io::{Read, Write},
    },
    tap::prelude::*,
    tempfile::tempfile,
};

pub struct WabbajackCDNDownloader {}

const MAGIC_FILENAME: &str = "definition.json.gz";

#[cfg(test)]
mod test_responses;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "PascalCase")]
#[serde(deny_unknown_fields)]
pub struct Part {
    pub hash: String,
    pub index: usize,
    pub offset: usize,
    pub size: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "PascalCase")]
#[serde(deny_unknown_fields)]
pub struct WabbajackCdnFile {
    pub author: String,
    pub server_assigned_unique_id: uuid::Uuid,
    pub hash: String,
    pub munged_name: String,
    pub original_file_name: String,
    pub size: u64,
    pub parts: Vec<Part>,
}

impl WabbajackCDNDownloader {
    pub async fn prepare_download(WabbajackCDNDownloaderState { url }: WabbajackCDNDownloaderState) -> Result<Vec<url::Url>> {
        let deduced_url = format!("{url}/{MAGIC_FILENAME}");
        Client::new()
            .get(deduced_url.to_string())
            .send()
            .map_with_context(|| format!("fetching from [{deduced_url}]"))
            .and_then(|response| response.bytes().map_context("reading bytes"))
            .and_then(|bytes| {
                tokio::task::spawn_blocking(move || {
                    GzDecoder::new(std::io::Cursor::new(&bytes)).pipe_ref_mut(|gzip| {
                        String::new()
                            .pipe(|mut output| {
                                gzip.read_to_string(&mut output)
                                    .context("decomplessing gzip archive")
                                    .with_context(|| {
                                        let maybe_string = String::from_utf8_lossy(&bytes);
                                        format!("some context in uncompressed response maybe?:\n'{maybe_string}'")
                                    })
                                    .map(|_| output)
                            })
                            .and_then(|contents| serde_json::from_str::<WabbajackCdnFile>(&contents).with_context(|| format!("deserializing:\n\n{contents}")))
                    })
                })
                .map_context("thread crashed")
                .and_then(ready)
            })
            .map_ok({
                let url = url.clone();
                move |WabbajackCdnFile {
                          author,
                          server_assigned_unique_id,
                          hash,
                          munged_name,
                          original_file_name,
                          size,
                          parts,
                      }| {
                    parts
                        .into_iter()
                        .map(move |Part { index, .. }| {
                            url.clone()
                                .tap_mut(|url| url.set_path(&format!("{munged_name}/parts/{index}")))
                        })
                        .collect_vec()
                }
            })
            .await
            .with_context(|| format!("fetching stuff from deduced url: [{deduced_url}] based on [{url}]"))
    }
}
