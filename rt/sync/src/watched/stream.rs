use {
    super::{Rx, RxError, Tx},
    futures_core::stream::{FusedStream, Stream},
    futures_util::{future::FutureExt, ready, stream},
    std::task::Poll,
    tokio_util::sync::ReusableBoxFuture,
};

pub type StaticStreamBox<T> = Box<dyn FusedStream<Item = T> + Unpin + Send + Sync + 'static>;
pub type WatchStreamBox<K, T> = StaticStreamBox<(K, Rx<T>)>;

async fn changed_static<T: 'static>(mut watch: Rx<T>) -> Result<Rx<T>, RxError> {
    watch.changed().await.map(move |()| watch)
}

fn watch_next_change<'a, K: 'a, T: 'static>(
    key: K,
    rx: Rx<T>,
) -> impl Stream<Item = (K, Rx<T>)> + Unpin + Send + Sync + 'a
where
    T: Sync + Send,
    K: Sync + Send + Clone,
{
    let mut key = Some(key);
    let mut storage = Some(ReusableBoxFuture::new(changed_static(rx)));
    stream::poll_fn(move |cx| {
        let Some(changed) = &mut storage else { return Poll::Pending };
        let res = ready!(changed.poll_unpin(cx));
        match res {
            Ok(watch) => {
                changed.set(changed_static(watch.clone()));
                Poll::Ready(key.clone().map(|k| (k, watch)))
            },
            Err(..) => {
                let _ = storage.take();
                Poll::Ready(None)
            },
        }
    })
}

pub fn stream_watch_changes_of<'t, K, T, I>(senders: I, mark_changed: bool) -> WatchStreamBox<K, T>
where
    I: IntoIterator<Item = (K, &'t Tx<T>)>,
    K: Sync + Send + Clone + 'static,
    T: Sync + Send + 'static,
{
    let stream = stream_watch_changes(senders.into_iter().map(|(key, tx)| {
        let mut rx = tx.subscribe();
        if mark_changed {
            rx.mark_changed();
        }
        (key, rx)
    }));
    Box::new(stream)
}

pub fn stream_watch_changes<'k, 'i, K, T, I>(
    senders: I,
) -> impl FusedStream<Item = (K, Rx<T>)> + Unpin + Send + Sync + 'k
where
    I: IntoIterator<Item = (K, Rx<T>)> + 'i,
    K: Sync + Send + Clone + 'k,
    T: Sync + Send + 'static,
{
    senders
        .into_iter()
        .map(|(key, rx)| watch_next_change(key, rx))
        .collect::<stream::SelectAll<_>>()
}
