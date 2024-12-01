use std::future::ready;
use tap::prelude::*;

use anyhow::{Context, Result};

pub struct GoogleDriveDownloader {}

impl GoogleDriveDownloader {
    /// wget --no-check-certificate 'https://docs.google.com/uc?export=download&id=1WmGuPCblM-L22O38qs939FRRs9ehnLsU' -O your_file_name
    pub fn download(id: String) -> Result<url::Url> {
        format!("https://docs.google.com/uc?export=download&id={id}")
            .pipe_deref(url::Url::parse)
            .context("could not build valid url")
    }
}
