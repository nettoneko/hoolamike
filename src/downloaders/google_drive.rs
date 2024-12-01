use {
    anyhow::{Context, Result},
    soup::NodeExt,
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

    #[derive(Debug)]
    struct FileURLRetrievalError(String);

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
                    .await
                    .context("extracting text")
                    .and_then(|text| response_parsing::get_url_from_gdrive_confirmation(&text))
                // // .tap(|e| panic!("{e}"))
                // .pipe_deref(soup::Soup::new)
                // .pipe(|response| -> Result<_> {
                //     // let href = response
                //     //     .tag("a")
                //     //     .find_all()
                //     //     .filter_map(|a| a.attrs().get("href").cloned())
                //     //     .find(|href| href.starts_with("/open"))
                //     //     .context("could not find href")?
                //     //     .replace("\\u003d", "=")
                //     //     .replace("%3F", "?")
                //     //     .replace("\\u0026", "&");
                //     let params = response
                //         .tag("input")
                //         .find_all()
                //         .filter_map(|input| {
                //             input.attrs().pipe_ref(|attrs| {
                //                 attrs
                //                     .get("type")
                //                     .and_then(|ty| ty.eq("hidden").then_some(()))
                //                     .and_then(|_| {
                //                         attrs
                //                             .get("name")
                //                             .cloned()
                //                             .zip(attrs.get("value").cloned())
                //                     })
                //             })
                //         })
                //         .collect::<BTreeMap<_, _>>();
                //     let params = serde_urlencoded::to_string(&params)
                //         .context("serializing query params")?;
                //     original_url
                //         .tap_mut(|original_url| {
                //             original_url.set_path("/open");
                //             original_url.set_query(Some(&params));
                //         })
                //         .pipe(Ok)
                // })
                //
            }
        }

        // .pipe_deref(url::Url::parse)
        // .context("could not build valid url")
    }
}
