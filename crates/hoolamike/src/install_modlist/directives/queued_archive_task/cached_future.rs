use {
    std::{collections::HashMap, future::Future, sync::Arc},
    tap::prelude::*,
    tokio::{sync::Mutex, task::JoinHandle},
    tracing::{error, instrument, trace, trace_span, warn, Instrument},
};

pub enum CacheState<T> {
    InProgress(Arc<tokio::sync::Notify>),
    Ready(Arc<T>),
}

impl<T> Clone for CacheState<T> {
    fn clone(&self) -> Self {
        match self {
            CacheState::InProgress(arc) => Self::InProgress(arc.clone()),
            CacheState::Ready(arc) => Self::Ready(arc.clone()),
        }
    }
}

pub struct CachedFutureQueue<K, V> {
    pub tasks: Mutex<HashMap<K, CacheState<V>>>,
}

impl<K, V> CachedFutureQueue<K, V>
where
    K: std::hash::Hash + Eq + std::fmt::Debug + Clone + Send + Sync + 'static,
    V: Send + Sync + 'static,
{
    pub fn new() -> Arc<Self> {
        Arc::new(Self { tasks: Default::default() })
    }

    pub fn get_spawn<F, Fut>(self: Arc<Self>, key: K, with: F) -> JoinHandle<Arc<V>>
    where
        Fut: Future<Output = V> + Send + 'static,
        F: Fn(K) -> Fut + Clone + Send + Sync + 'static,
    {
        tokio::task::spawn(self.get(key, with))
    }

    #[instrument(skip(self, with), level = "TRACE")]
    pub async fn get<F, Fut>(self: Arc<Self>, key: K, with: F) -> Arc<V>
    where
        Fut: Future<Output = V>,
        F: FnOnce(K) -> Fut + Send + 'static,
    {
        let mut lock = self.tasks.lock().await;
        let current = lock.get(&key).cloned();
        match current {
            Some(occupied_entry) => {
                trace!("entry already exists");
                match occupied_entry {
                    CacheState::Ready(already_exists) => {
                        trace!("value already cached ");
                        already_exists
                    }
                    CacheState::InProgress(waiting) => {
                        trace!("task in progress, waiting for notification about progress");
                        drop(lock);
                        waiting
                            .notified()
                            .instrument(trace_span!("operation in progress, awaiting it's completion"))
                            .await;
                        trace!("notified of progress, checking again");
                        self.clone()
                            .get(key.clone(), with)
                            .instrument(trace_span!("getting another archive because got notified"))
                            .pipe(Box::pin)
                            .await
                    }
                }
            }
            None => {
                trace!("entry does not exist, setting up notifier before starting work");
                lock.insert(
                    key.clone(),
                    tokio::sync::Notify::new()
                        .pipe(Arc::new)
                        .clone()
                        .pipe(CacheState::InProgress),
                )
                .pipe(drop);
                drop(lock);
                trace!("starting work");
                let res = (with)(key.clone()).await.pipe(Arc::new);
                trace!("work finished");
                let current = self
                    .clone()
                    .tasks
                    .lock()
                    .await
                    .insert(key.clone(), res.clone().pipe(CacheState::Ready));
                match current {
                    Some(CacheState::InProgress(notify)) => {
                        trace!("notifying waiters");
                        notify.notify_one();
                        notify.notify_waiters()
                    }
                    Some(CacheState::Ready(_)) => {
                        warn!("duplicated work detected")
                    }
                    None => error!("nobody was waiting for this task?"),
                };
                trace!("returning value");
                res
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        futures::{FutureExt, StreamExt, TryStreamExt},
        tokio::time::{sleep, Duration},
        tracing::{info, info_span},
    };

    #[test_log::test(tokio::test)]
    async fn test_simple() -> anyhow::Result<()> {
        let queue = CachedFutureQueue::new();
        let slow_times_two = |num| async move {
            info!("sleeping for 100ms");
            sleep(Duration::from_millis(100)).await;
            info!("slept, returning result");
            num * 2
        };
        info!("spawning task_1");
        let task_1 = queue
            .clone()
            .get_spawn(1, slow_times_two)
            .instrument(info_span!("task_1"))
            .inspect(|res| info!(?res, "task_1 finished"));
        sleep(Duration::from_millis(80)).await;
        info!("spawning task_2");
        let task_2 = queue
            .get_spawn(1, slow_times_two)
            .instrument(info_span!("task_2"))
            .inspect(|res| info!(?res, "task_2 finished"));

        info!("joining");
        let finished = task_1
            .into_stream()
            .chain(task_2.into_stream())
            .try_collect::<Vec<_>>()
            .await?;
        let [a, b] = finished.as_slice() else { panic!("bad task count") };
        info!("results: (a={a}, b={b})");

        assert!(Arc::ptr_eq(a, b));
        Ok(())
    }
}
