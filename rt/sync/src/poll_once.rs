use {
    core::{
        future::Future,
        mem,
        pin::Pin,
        sync::atomic::{AtomicBool, Ordering},
        task::{self, Poll},
    },
    std::{borrow::Cow, sync::Arc, task::Wake},
};

pub struct PollOnce<F> {
    f: F,
}

impl<F> PollOnce<F> {
    pub fn new(f: F) -> Self {
        Self { f }
    }

    pub fn inner_mut(self: Pin<&mut Self>) -> Pin<&mut F> {
        unsafe { self.map_unchecked_mut(|this| &mut this.f) }
    }
}

impl<F: Unpin> PollOnce<F> {
    pub fn pinned(&mut self) -> Pin<&mut Self> {
        Pin::new(self)
    }
}

impl<F: Future> PollOnce<F> {
    pub fn poll_with_limit(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context,
        iterations: usize,
    ) -> Poll<Option<F::Output>> {
        for _ in 0..iterations {
            match self.as_mut().poll(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Poll::Ready(res)) => return Poll::Ready(Some(res)),
                Poll::Ready(Poll::Pending) => (),
            }
        }
        Poll::Ready(None)
    }

    pub fn with_limit<'a>(
        mut self: Pin<&'a mut Self>,
        mut iterations: usize,
    ) -> impl Future<Output = Option<F::Output>> + 'a {
        futures_util::future::poll_fn(move |cx| {
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
        let waker = unsafe { task::Waker::from_raw(self.raw_waker()) };
        f(&waker)
    }

    #[inline]
    fn waker_to_raw(waker: task::Waker) -> task::RawWaker {
        unsafe { mem::transmute(waker) }
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
        let upstream_waker =
            mem::replace(&mut self.upstream_waker, Cow::Owned(task::Waker::noop().clone()));
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

impl<'w> Wake for ReenterWaker<'w> {
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
