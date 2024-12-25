use {
    super::helpers::FutureAnyhowExt,
    anyhow::{Context, Result},
    futures::TryFutureExt,
    std::future::ready,
};

pub struct MediaFireDownloader {}

pub mod response_parsing {
    use {
        anyhow::{Context, Result},
        scraper::{Html, Selector},
        tap::prelude::*,
        url::Url,
    };

    /// BASED ON https://github.com/wkentaro/gdown/blob/main/gdown/download.py
    pub fn get_url_from_gdrive_confirmation(contents: &str) -> Result<url::Url> {
        Selector::parse("a.downloadButton")
            .map_err(|e| anyhow::anyhow!("{e:?}"))
            .context("bad selector")
            .and_then(|a| {
                a.pipe_ref(|a| {
                    contents
                        .pipe(Html::parse_document)
                        .select(a)
                        .next()
                        .context("no 'a.downloadButton' on page")
                        .and_then(|button| {
                            button
                                .attr("href")
                                .context("button has no href")
                                .and_then(|url| Url::parse(url).with_context(|| format!("parsing [{url}]")))
                        })
                })
            })
    }
}

impl MediaFireDownloader {
    pub async fn download(url: url::Url) -> Result<url::Url> {
        reqwest::Client::new()
            .get(url.to_string())
            .send()
            .await
            .context("fetching the media fire response")?
            .error_for_status()
            .context("bad status")?
            .text()
            .map_context("extracting text")
            .and_then(|text| {
                tokio::task::spawn_blocking(move || response_parsing::get_url_from_gdrive_confirmation(&text))
                    .map_context("thread crashed")
                    .and_then(ready)
            })
            .await
    }
}
