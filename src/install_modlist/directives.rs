use {
    crate::error::{MultiErrorCollectExt, TotalResult},
    anyhow::{Context, Result},
    futures::StreamExt,
    std::sync::Arc,
    tap::prelude::*,
};

pub mod create_bsa {
    use {super::*, crate::modlist_json::directive::CreateBSADirective};

    #[derive(Clone, Debug)]
    pub struct CreateBSAHandler {}

    impl CreateBSAHandler {
        pub fn handle(self, directive: CreateBSADirective) -> Result<()> {
            anyhow::bail!("[CreateBSADirective ] {directive:#?} is not implemented")
        }
    }
}

pub mod from_archive {
    use {super::*, crate::modlist_json::directive::FromArchiveDirective};

    #[derive(Clone, Debug)]
    pub struct FromArchiveHandler {}

    impl FromArchiveHandler {
        pub fn handle(self, directive: FromArchiveDirective) -> Result<()> {
            anyhow::bail!("[FromArchiveDirective ] {directive:#?} is not implemented")
        }
    }
}

pub mod inline_file {
    use {super::*, crate::modlist_json::directive::InlineFileDirective};

    #[derive(Clone, Debug)]
    pub struct InlineFileHandler {}

    impl InlineFileHandler {
        pub fn handle(self, directive: InlineFileDirective) -> Result<()> {
            anyhow::bail!("[InlineFileDirective ] {directive:#?} is not implemented")
        }
    }
}

pub mod patched_from_archive {
    use {super::*, crate::modlist_json::directive::PatchedFromArchiveDirective};

    #[derive(Clone, Debug)]
    pub struct PatchedFromArchiveHandler {}

    impl PatchedFromArchiveHandler {
        pub fn handle(self, directive: PatchedFromArchiveDirective) -> Result<()> {
            anyhow::bail!("[PatchedFromArchiveDirective ] {directive:#?} is not implemented")
        }
    }
}

pub mod remapped_inline_file {
    use {super::*, crate::modlist_json::directive::RemappedInlineFileDirective};

    #[derive(Clone, Debug)]
    pub struct RemappedInlineFileHandler {}

    impl RemappedInlineFileHandler {
        pub fn handle(self, directive: RemappedInlineFileDirective) -> Result<()> {
            anyhow::bail!("[RemappedInlineFileDirective ] {directive:#?} is not implemented")
        }
    }
}

pub mod transformed_texture {
    use {super::*, crate::modlist_json::directive::TransformedTextureDirective};

    #[derive(Clone, Debug)]
    pub struct TransformedTextureHandler {}

    impl TransformedTextureHandler {
        pub fn handle(self, directive: TransformedTextureDirective) -> Result<()> {
            anyhow::bail!("[TransformedTextureDirective ] {directive:#?} is not implemented")
        }
    }
}

use crate::modlist_json::Directive;

pub struct DirectivesHandler {
    pub create_bsa: create_bsa::CreateBSAHandler,
    pub from_archive: from_archive::FromArchiveHandler,
    pub inline_file: inline_file::InlineFileHandler,
    pub patched_from_archive: patched_from_archive::PatchedFromArchiveHandler,
    pub remapped_inline_file: remapped_inline_file::RemappedInlineFileHandler,
    pub transformed_texture: transformed_texture::TransformedTextureHandler,
}

impl DirectivesHandler {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            create_bsa: create_bsa::CreateBSAHandler {},
            from_archive: from_archive::FromArchiveHandler {},
            inline_file: inline_file::InlineFileHandler {},
            patched_from_archive: patched_from_archive::PatchedFromArchiveHandler {},
            remapped_inline_file: remapped_inline_file::RemappedInlineFileHandler {},
            transformed_texture: transformed_texture::TransformedTextureHandler {},
        }
    }
    pub async fn handle(self: Arc<Self>, directive: Directive) -> Result<()> {
        match directive {
            Directive::CreateBSA(directive) => self.create_bsa.clone().handle(directive),
            Directive::FromArchive(directive) => self.from_archive.clone().handle(directive),
            Directive::InlineFile(directive) => self.inline_file.clone().handle(directive),
            Directive::PatchedFromArchive(directive) => self.patched_from_archive.clone().handle(directive),
            Directive::RemappedInlineFile(directive) => self.remapped_inline_file.clone().handle(directive),
            Directive::TransformedTexture(directive) => self.transformed_texture.clone().handle(directive),
        }
    }

    pub async fn handle_directives(self: Arc<Self>, directives: Vec<Directive>) -> TotalResult<()> {
        directives
            .pipe(futures::stream::iter)
            .then(|directive| self.clone().handle(directive))
            .collect::<Vec<Result<_>>>()
            .await
            .multi_error_collect()
    }
}
