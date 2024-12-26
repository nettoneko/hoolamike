use {
    super::helpers::FutureAnyhowExt,
    crate::modlist_json::{HumanUrl, WabbajackCDNDownloaderState},
    anyhow::{Context, Result},
    flate2::read::GzDecoder,
    futures::TryFutureExt,
    itertools::Itertools,
    reqwest::Client,
    serde::{Deserialize, Serialize},
    std::{future::ready, io::Read},
    tap::prelude::*,
    url::Url,
};

pub struct WabbajackCDNDownloader {}

const MAGIC_FILENAME: &str = "definition.json.gz";

#[cfg(test)]
mod test_responses;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct Part {
    pub hash: String,
    pub index: usize,
    pub offset: usize,
    pub size: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct WabbajackCdnFile {
    pub author: String,
    pub server_assigned_unique_id: Option<uuid::Uuid>,
    pub hash: String,
    pub munged_name: String,
    pub original_file_name: String,
    pub size: u64,
    pub parts: Vec<Part>,
}

#[test]
fn test_example_cdn_files() -> Result<()> {
    #[rustfmt::skip]
    const EXAMPLES: &[&str] = &[
        r#"{"$type":"CDNFileDefinition, Wabbajack.Lib","Author":"lively","OriginalFileName":"splash.7z","Size":87967,"Hash":"eOiJRFeBuzY=","Parts":[{"$type":"CDNFilePartDefinition, Wabbajack.Lib","Size":87967,"Offset":0,"Hash":"eOiJRFeBuzY=","Index":0}],"ServerAssignedUniqueId":"3075fd8a-1970-45b4-be96-a82463f37f3f","MungedName":"splash.7z_3075fd8a-1970-45b4-be96-a82463f37f3f"}"#,
        include_str!("wabbajack_cdn/test_raw_responses/bad-response-1.bytes"),

    ];
    EXAMPLES
        .iter()
        .try_for_each(|example| parse_wabbajack_cdn_file_response(example).map(|_| ()))
}

pub fn remap_wabbajack_cdn_url(url: Url) -> Result<url::Url> {
    url.to_string()
        .pipe(|url| {
            [
                ("wabbajack.b-cdn.net", "authored-files.wabbajack.org"),
                ("wabbajack-mirror.b-cdn.net", "mirror.wabbajack.org"),
                ("wabbajack-patches.b-cdn.net", "patches.wabbajack.org"),
                ("wabbajacktest.b-cdn.net", "test-files.wabbajack.org"),
            ]
            .into_iter()
            .fold(url, |url, (from, to)| url.replace(from, to))
        })
        .pipe_deref(url::Url::parse)
        .context("remapping url doesnt work")
}

fn parse_wabbajack_cdn_file_response(contents: &str) -> Result<WabbajackCdnFile> {
    Err(())
        .or_else(|e| {
            serde_json::from_str::<WabbajackCdnFile>(contents.trim())
                .context("checking just object")
                .with_context(|| format!("trying because: {e:#?}"))
        })
        .or_else(|e| {
            serde_json::from_str::<WabbajackCdnFile>(
                // some wild unicode in the response
                &contents
                    .trim()
                    .chars()
                    .skip_while(|c| c != &'{')
                    .collect::<String>(),
            )
            .context("checking checking object with wild unicode characters")
            .with_context(|| format!("trying because: {e:#?}"))
        })
        .with_context(|| {
            format!(
                "deserializing:\n\n{contents:#?}\n\n({:#?})",
                serde_json::from_str::<serde_json::Value>(contents.trim())
            )
        })
        .context("invalid wabbajack cdn response")
}

impl WabbajackCDNDownloader {
    pub async fn prepare_download(WabbajackCDNDownloaderState { url }: WabbajackCDNDownloaderState) -> Result<Vec<HumanUrl>> {
        let url = url
            .clone()
            .conv::<url::Url>()
            .pipe(remap_wabbajack_cdn_url)?
            .conv::<HumanUrl>();

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
                                    .context("decompressing gzipped contents")
                                    .with_context(|| {
                                        let maybe_string = String::from_utf8_lossy(&bytes);
                                        format!("some context in uncompressed response maybe?:\n'{maybe_string}'")
                                    })
                                    .map(|_| output)
                            })
                            .and_then(|contents| parse_wabbajack_cdn_file_response(&contents))
                    })
                })
                .map_context("thread crashed")
                .and_then(ready)
            })
            .map_ok({
                let url = url.clone();
                move |WabbajackCdnFile {
                          munged_name,
                          parts,
                          author: _,
                          server_assigned_unique_id: _,
                          hash: _,
                          original_file_name: _,
                          size: _,
                      }| {
                    parts
                        .into_iter()
                        .map(move |Part { index, .. }| {
                            url.clone().tap_mut(|url| {
                                url.as_mut()
                                    .set_path(&format!("{munged_name}/parts/{index}"))
                            })
                        })
                        .collect_vec()
                }
            })
            .await
            .with_context(|| format!("fetching stuff from deduced url: [{deduced_url}] based on [{url}]"))
    }
}
