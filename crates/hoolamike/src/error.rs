use {
    futures::{FutureExt, Stream, StreamExt},
    std::future::ready,
    tap::prelude::*,
};
pub type TotalResult<T> = std::result::Result<Vec<T>, Vec<anyhow::Error>>;

#[extension_traits::extension(pub(crate) trait MultiErrorCollectExt)]
impl<S, T> S
where
    S: Stream<Item = anyhow::Result<T>> + StreamExt,
{
    async fn multi_error_collect(self) -> TotalResult<T> {
        self.fold((vec![], vec![]), |acc, next| {
            acc.tap_mut(|(ok, errors)| match next {
                Ok(v) => ok.push(v),
                Err(e) => errors.push(e),
            })
            .pipe(ready)
        })
        .map(|(ok, errors)| errors.is_empty().then_some(ok).ok_or(errors))
        .await
    }
}
