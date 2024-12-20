use {
    super::*,
    crate::modlist_json::{directive::CreateBSADirective, DirectiveState, FileState},
    std::any::Any,
};

#[derive(Clone, Debug)]
pub struct CreateBSAHandler {}

impl CreateBSAHandler {
    pub async fn handle(
        self,
        CreateBSADirective {
            hash,
            size,
            to,
            temp_id,
            file_states,
            state,
        }: CreateBSADirective,
    ) -> Result<u64> {
        anyhow::bail!("ooo")
        // tokio::task::spawn_blocking(move || {
        //     file_states.into_iter().fold(
        //         ba2::fo4::Archive::new(),
        //         |archive,
        //          FileState {
        //              file_state_type,
        //              align,
        //              compressed,
        //              dir_hash,
        //              chunk_hdr_len,
        //              chunks,
        //              num_mips,
        //              pixel_format,
        //              tile_mode,
        //              unk_8,
        //              extension,
        //              height,
        //              width,
        //              is_cube_map,
        //              flags,
        //              index,
        //              name_hash,
        //              path,
        //          }| {},
        //     )
        // })
    }
}
