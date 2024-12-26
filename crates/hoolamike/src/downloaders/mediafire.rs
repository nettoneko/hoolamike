use {
    super::helpers::FutureAnyhowExt,
    crate::modlist_json::HumanUrl,
    anyhow::{Context, Result},
    futures::TryFutureExt,
    std::future::ready,
    tap::prelude::*,
    tracing::instrument,
};

pub struct MediaFireDownloader {}

pub mod response_parsing {
    use {
        crate::modlist_json::HumanUrl,
        anyhow::{Context, Result},
        scraper::{Html, Selector},
        std::str::FromStr,
    };

    /// BASED ON https://github.com/wkentaro/gdown/blob/main/gdown/download.py
    pub fn get_url_from_mediafire_confirmation(contents: &str) -> Result<HumanUrl> {
        Selector::parse("input.popsok[aria-label='Download file']")
            .map_err(|e| anyhow::anyhow!("{e:?}"))
            .context("parsing selector")
            .and_then(|selector| {
                Err(anyhow::anyhow!("finding any url"))
                    .or_else(|cause| {
                        Html::parse_document(contents)
                            .select(&selector)
                            .next()
                            .context("selector matched nothing")
                            .and_then(|input| input.attr("href").context("no href"))
                            .and_then(|href| HumanUrl::from_str(href).with_context(|| format!("bad url: {href}")))
                            .context("trying the selector method")
                            .with_context(|| format!("trying because: {cause:?}"))
                    })
                    .or_else(|cause| {
                        let start_text = "window.location.href = '";
                        contents
                            .find(start_text)
                            .with_context(|| format!("'{start_text}' not found"))
                            .and_then(|start| {
                                contents
                                    .get(start..)
                                    .with_context(|| format!("invalid subslice: {start}.."))
                            })
                            .map(|slice| slice.chars().take_while(|c| c != &'\'').collect::<String>())
                            .and_then(|url| HumanUrl::from_str(&url).with_context(|| format!("bad url: {url}")))
                            .context("trying the substring method")
                            .with_context(|| format!("trying becasue: {cause:?}"))
                    })
            })
        // Selector::parse("a.downloadButton")
        //     .map_err(|e| anyhow::anyhow!("{e:?}"))
        //     .context("bad selector")
        //     .and_then(|a| {
        //         a.pipe_ref(|a| {
        //             contents
        //                 .pipe(Html::parse_document)
        //                 .select(a)
        //                 .next()
        //                 .context("no 'a.downloadButton' on page")
        //                 .and_then(|button| {
        //                     button
        //                         .attr("href")
        //                         .context("button has no href")
        //                         .and_then(|url| Url::parse(url).with_context(|| format!("parsing [{url}]")))
        //                 })
        //         })
        //     })
    }
}

impl MediaFireDownloader {
    #[instrument]
    pub async fn download(url: HumanUrl) -> Result<HumanUrl> {
        reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/109.0.0.0 Safari/537.36")
            .build()
            .context("bad http client")?
            .get(url.to_string())
            .send()
            .map_context("fetching the media fire response")
            // .and_then(|res| {
            //     res.error_for_status()
            //         .context("bad status code")
            //         .pipe(ready)
            // })
            .and_then(|res| res.text().map_context("extracting text"))
            .and_then(|text| {
                tokio::task::spawn_blocking(move || {
                    response_parsing::get_url_from_mediafire_confirmation(&text).tap_ok(|url| tracing::debug!(%url, "parsed mediafire url"))
                })
                .map_context("thread crashed")
                .and_then(ready)
            })
            .await
            .with_context(|| format!("preparing MediaFire download for [{url}]"))
    }
}
