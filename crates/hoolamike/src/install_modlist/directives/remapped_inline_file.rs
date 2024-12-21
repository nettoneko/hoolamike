use {super::*, crate::modlist_json::directive::RemappedInlineFileDirective};

#[derive(Clone, Debug)]
pub struct RemappedInlineFileHandler {}

impl RemappedInlineFileHandler {
    pub async fn handle(self, directive: RemappedInlineFileDirective) -> Result<u64> {
        anyhow::bail!("[RemappedInlineFileDirective ] {directive:#?} is not implemented")
    }
}
