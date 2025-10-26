use {
    anyhow::Context,
    crate::{
        controller::Controller,
        render::{
            machine::{RenderMachine, RenderTask, RenderTaskPriority},
            RenderState,
        },
    },
    std::{
        borrow::Cow,
        future::Future,
        mem,
        pin::Pin,
        ptr,
        sync::{atomic::{AtomicBool, Ordering}, Arc},
        task::{self, Poll},
        time::Duration,
    },
    tokio::{
        runtime::{Builder, Handle, Runtime},
        sync::oneshot,
        task::LocalSet,
    },
};

impl Controller {
    pub const RUNTIME_BLOCKING_TIMEOUT: Duration = Duration::from_secs(12);
    pub const RUNTIME_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(8);

    pub fn new_runtime() -> anyhow::Result<Runtime> {
        let runtime = Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .max_blocking_threads(12)
            .thread_keep_alive(Self::RUNTIME_BLOCKING_TIMEOUT)
            .thread_name("taimi-controller")
            .build()
            .context("Async runtime initialization")?;
        Ok(runtime)
    }

    pub fn shutdown(runtime: Runtime) {
        runtime.shutdown_timeout(Self::RUNTIME_SHUTDOWN_TIMEOUT);
    }

    pub async fn schedule_render<R, F: FnOnce(&mut RenderState) -> R>(prio: RenderTaskPriority, f: F) -> oneshot::Receiver<R> where
        F: Send + 'static,
        R: Send + 'static,
    {
        let (tx, rx) = oneshot::channel::<R>();
        let task = Box::new(move |state: &'_ mut RenderState| {
            let res = f(state);
            let res = tx.send(res).map_err(drop);
            if res.is_err() {
                log::debug!("render task receiver dropped");
            }
        }) as RenderTask;

        RenderMachine::schedule_task_async(task, prio).await;
        rx
    }

    pub async fn run_render<R, F: FnOnce(&mut RenderState) -> R>(prio: RenderTaskPriority, f: F) -> Result<R, oneshot::error::RecvError> where
        F: Send + 'static,
        R: Send + 'static,
    {
        Self::schedule_render(prio, f).await.await
    }

    /// Bring in the render thread onto the runtime
    #[cfg(feature = "render-rt")]
    pub(crate) fn render_inherit(&mut self) {
        let handle = Handle::current();

        let task = Box::new(move |state: &'_ mut RenderState| {
            let context = RemoteContext::new(handle);
            state.runtime = Some(context);
        }) as RenderTask;
        tokio::spawn(RenderMachine::schedule_task_async(task, RenderTaskPriority::Immediate));
    }
    #[cfg(not(feature = "render-rt"))]
    pub(crate) fn render_inherit(&mut self) {}
}

pub struct RemoteContext {
    pub handle: Handle,
    local_set: Option<ptr::NonNull<LocalSet>>,
}

impl RemoteContext {
    pub const fn new(handle: Handle) -> Self {
        Self {
            handle,
            local_set: None,
        }
    }

    pub fn render_local_set(&mut self) -> Option<&mut Option<Box<LocalSet>>> {
        if RenderState::is_render_thread() {
            Some(unsafe {
                self.render_local_set_unchecked()
            })
        } else {
            None
        }
    }

    pub unsafe fn render_local_set_unchecked(&mut self) -> &mut Option<Box<LocalSet>> {
        &mut *(&mut self.local_set as *mut Option<_> as *mut Option<Box<LocalSet>>)
    }
}
unsafe impl Send for RemoteContext {}
unsafe impl Sync for RemoteContext {}

impl Drop for RemoteContext {
    fn drop(&mut self) {
        match self.render_local_set() {
            #[cfg(todo = "unnecessary")]
            Some(None) => (),
            Some(locals) => {
                log::debug!("RemoteContext dropping LocalSet...");
                let _ = locals.take();
            },
            None => {
                log::debug!("RemoteContext dropped on background thread?");
            },
        }
    }
}

pub struct PollOnce<F> {
    f: F,
}

impl<F> PollOnce<F> {
    pub fn new(f: F) -> Self {
        Self {
            f,
        }
    }

    pub fn inner_mut(self: Pin<&mut Self>) -> Pin<&mut F> {
        unsafe {
            self.map_unchecked_mut(|this| &mut this.f)
        }
    }
}

impl<F: Unpin> PollOnce<F> {
    pub fn pinned(&mut self) -> Pin<&mut Self> {
        Pin::new(self)
    }
}

impl<F: Future> PollOnce<F> {
    pub fn poll_with_limit(mut self: Pin<&mut Self>, cx: &mut task::Context, iterations: usize) -> Poll<Option<F::Output>> {
        for _ in 0..iterations {
            match self.as_mut().poll(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Poll::Ready(res)) => return Poll::Ready(Some(res)),
                Poll::Ready(Poll::Pending) => (),
            }
        }
        Poll::Ready(None)
    }

    pub fn with_limit<'a>(mut self: Pin<&'a mut Self>, mut iterations: usize) -> impl Future<Output = Option<F::Output>> + 'a {
        futures::future::poll_fn(move |cx| {
            let res = self.as_mut().poll_with_limit(cx, iterations);
            iterations = 0;
            res
        })
    }
}

impl<F: Future> Future for PollOnce<F> {
    type Output = Poll<F::Output>;

    fn poll(self: Pin<&mut Self>, cx: &mut task::Context) -> Poll<Self::Output> {
        let waker = ReenterWaker::new_borrowed(cx.waker());
        let res = waker.with_waker(|waker| {
            let mut wrapped = task::Context::from_waker(waker);
            self.inner_mut().poll(&mut wrapped)
        });
        match res {
            Poll::Ready(res) => Poll::Ready(Poll::Ready(res)),
            Poll::Pending if waker.is_awake() => Poll::Ready(Poll::Pending),
            Poll::Pending => Poll::Pending,
        }
    }
}

pub struct ReenterWaker<'w> {
    pub upstream_waker: Cow<'w, task::Waker>,
    pub awake: AtomicBool,
    pub reproduce: bool,
}

impl<'w> ReenterWaker<'w> {
    #[inline]
    pub const fn new(upstream_waker: Cow<'w, task::Waker>) -> Self {
        Self {
            upstream_waker,
            awake: AtomicBool::new(false),
            reproduce: false,
        }
    }

    pub const fn new_borrowed(upstream_waker: &'w task::Waker) -> Self {
        Self::new(Cow::Borrowed(upstream_waker))
    }

    pub fn to_owned(&self) -> ReenterWaker<'static> {
        ReenterWaker::new(Cow::Owned((*self.upstream_waker).clone()))
    }

    pub const RAW_WAKER_VTABLE: &'static task::RawWakerVTable = &task::RawWakerVTable::new(
        Self::raw_clone,
        Self::raw_wake,
        Self::raw_wake_by_ref,
        Self::raw_drop,
    );

    pub fn raw_waker(&self) -> task::RawWaker {
        task::RawWaker::new(self as *const Self as *const (), Self::RAW_WAKER_VTABLE)
    }

    pub fn with_waker<R, F: FnOnce(&task::Waker) -> R>(&self, f: F) -> R {
        let waker = unsafe {
            task::Waker::from_raw(self.raw_waker())
        };
        f(&waker)
    }

    #[inline]
    fn waker_to_raw(waker: task::Waker) -> task::RawWaker {
        unsafe {
            mem::transmute(waker)
        }
    }

    #[inline(never)]
    pub unsafe fn raw_clone(waker: *const ()) -> task::RawWaker {
        let waker = &*(waker as *const Self);
        Self::waker_to_raw(match waker.reproduce {
            true => task::Waker::from(Arc::new(waker.to_owned())),
            false => (*waker.upstream_waker).clone(),
        })
    }

    #[inline(never)]
    pub unsafe fn raw_wake(waker: *const ()) {
        let waker = &*(waker as *const Self);
        waker.wake_ref()
    }

    #[inline(never)]
    pub unsafe fn raw_wake_by_ref(waker: *const ()) {
        let waker = &*(waker as *const Self);
        waker.wake_ref()
    }

    #[inline(never)]
    pub unsafe fn raw_drop(_waker: *const ()) {}

    pub fn wake_and_take(&mut self) {
        let upstream_waker = mem::replace(&mut self.upstream_waker, Cow::Owned(task::Waker::noop().clone()));
        match upstream_waker {
            Cow::Owned(waker) => waker.wake(),
            // unreachable
            Cow::Borrowed(waker) => waker.wake_by_ref(),
        }
    }

    pub fn wake_ref(&self) {
        self.awake.store(true, Ordering::Relaxed)
    }

    pub fn is_awake(&self) -> bool {
        self.awake.load(Ordering::Relaxed)
    }
}

impl<'w> task::Wake for ReenterWaker<'w> {
    fn wake(mut self: Arc<Self>) {
        {
            let this = match self.upstream_waker {
                Cow::Owned(..) => Arc::get_mut(&mut self),
                _ => None,
            };
            if let Some(this) = this {
                return this.wake_and_take()
            }
        }
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.wake_ref();
    }
}
