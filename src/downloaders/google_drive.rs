use {
    super::helpers::FutureAnyhowExt,
    anyhow::{Context, Result},
    futures::TryFutureExt,
    std::future::ready,
    tap::prelude::*,
};

pub struct GoogleDriveDownloader {}

pub mod response_parsing {
    use {
        anyhow::{Context, Result},
        regex::Regex,
        scraper::{Html, Selector},
        std::collections::HashMap,
        url::{form_urlencoded, Url},
    };

    /// BASED ON https://github.com/wkentaro/gdown/blob/main/gdown/download.py
    pub fn get_url_from_gdrive_confirmation(contents: &str) -> Result<url::Url> {
        let mut url = String::new();

        let download_url_re = Regex::new(r#"href="(\/uc\?export=download[^"]+)"#).unwrap();
        let json_download_url_re = Regex::new(r#""downloadUrl":"([^"]+)""#).unwrap();
        let error_caption_re = Regex::new(r#"<p class="uc-error-subcaption">(.*)</p>"#).unwrap();

        for line in contents.lines() {
            if let Some(captures) = download_url_re.captures(line) {
                url = format!("https://docs.google.com{}", captures.get(1).unwrap().as_str());
                url = url.replace("&amp;", "&");
                break;
            }

            let document = Html::parse_fragment(line);
            let form_selector = Selector::parse("#download-form").unwrap();
            if let Some(form) = document.select(&form_selector).next() {
                if let Some(action) = form.value().attr("action") {
                    url = action.replace("&amp;", "&");
                    let mut url_components = Url::parse(&url).context("Invalid URL format")?;
                    let mut query_params: HashMap<_, _> = url_components.query_pairs().into_owned().collect();

                    for input in form.select(&Selector::parse("input[type=\"hidden\"]").unwrap()) {
                        if let (Some(name), Some(value)) = (input.value().attr("name"), input.value().attr("value")) {
                            query_params.insert(name.to_string(), value.to_string());
                        }
                    }

                    let query = form_urlencoded::Serializer::new(String::new())
                        .extend_pairs(query_params)
                        .finish();
                    url_components.set_query(Some(&query));
                    url = url_components.to_string();
                    break;
                }
            }

            if let Some(captures) = json_download_url_re.captures(line) {
                url = captures.get(1).unwrap().as_str().to_string();
                url = url.replace("\\u003d", "=").replace("\\u0026", "&");
                break;
            }

            if let Some(captures) = error_caption_re.captures(line) {
                anyhow::bail!("{}", (captures.get(1).unwrap().as_str()))
            }
        }

        Url::parse(&url).context("could not retrieve the google drive file link from google prompt")
    }
}

impl GoogleDriveDownloader {
    /// wget --no-check-certificate 'https://docs.google.com/uc?export=download&id=1WmGuPCblM-L22O38qs939FRRs9ehnLsU' -O your_file_name
    pub async fn download(id: String, expected_size: u64) -> Result<url::Url> {
        let original_url = format!("https://docs.google.com/uc?export=download&id={id}&export=download&confirm=t")
            .pipe_deref(url::Url::parse)
            .context("invalid url")?;

        let response = {
            reqwest::Client::new()
                .get(original_url.to_string())
                .send()
                .await
                .context("fetching google drive warning page")?
                .error_for_status()
                .context("bad status")?
        };
        match response.content_length() {
            Some(size) if expected_size == size => Ok(original_url),
            _ => {
                response
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
    }
}
