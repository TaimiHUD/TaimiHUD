use {
    crate::{
        controller::Controller,
        exports::runtime as rt,
        render::{
            machine::{RenderMachine, RenderTask, RenderTaskPriority},
            RenderState,
        },
    },
    anyhow::Context,
    futures::future::Either,
    std::{
        fmt,
        ptr,
        sync::{LazyLock, RwLock},
        time::{Duration, Instant as StdInstant, SystemTime},
    },
    taimi_hoard::time::Timestamp,
    tokio::{
        runtime::{Builder, Handle, Runtime},
        sync::oneshot,
        task::LocalSet,
        time::{self, Instant},
    },
};

impl Controller {
    pub const RUNTIME_BLOCKING_TIMEOUT: Duration = Duration::from_secs(9);
    pub const RUNTIME_SHUTDOWN_TIMEOUT: Duration =
        Duration::from_secs(Self::RUNTIME_BLOCKING_TIMEOUT.as_secs() + 2);

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

    pub async fn schedule_render<R, F: FnOnce(&mut RenderState) -> R>(
        prio: RenderTaskPriority,
        f: F,
    ) -> oneshot::Receiver<R>
    where
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

    pub async fn run_render<R, F: FnOnce(&mut RenderState) -> R>(
        prio: RenderTaskPriority,
        f: F,
    ) -> Result<R, oneshot::error::RecvError>
    where
        F: Send + 'static,
        R: Send + 'static,
    {
        Self::schedule_render(prio, f).await.await
    }
    pub async fn try_run_render<R, F: FnOnce(&mut RenderState) -> anyhow::Result<R>>(
        prio: RenderTaskPriority,
        f: F,
    ) -> anyhow::Result<R>
    where
        F: Send + 'static,
        R: Send + 'static,
    {
        let res = Self::run_render(prio, f).await;
        flatten_result_with("render task lost", res)
    }

    pub async fn try_run_blocking<R, C, F: FnOnce() -> anyhow::Result<R>>(
        context: C,
        f: F,
    ) -> anyhow::Result<R>
    where
        R: Send + 'static,
        F: Send + 'static,
        C: fmt::Display,
    {
        let res = tokio::task::spawn_blocking(f)
            .await
            .with_context(|| format!("panicked: {context}"));
        flatten_result_any(res)
    }

    /// Bring in the render thread onto the runtime
    #[cfg(feature = "render-rt")]
    pub(crate) fn render_inherit(&mut self) {
        let handle = Handle::current();

        let task = Box::new(move |state: &'_ mut RenderState| {
            let context = RemoteContext::new(handle);
            state.runtime = Some(context);
        }) as RenderTask;
        tokio::spawn(RenderMachine::schedule_task_async(
            task,
            RenderTaskPriority::Immediate,
        ));
    }
    #[cfg(not(feature = "render-rt"))]
    pub(crate) fn render_inherit(&mut self) {}
}

pub fn flatten_result_any<T>(res: anyhow::Result<anyhow::Result<T>>) -> anyhow::Result<T> {
    match res {
        Ok(res) => res,
        Err(e) => Err(e),
    }
}
pub fn flatten_result<T, E: Into<anyhow::Error>>(res: Result<anyhow::Result<T>, E>) -> anyhow::Result<T> {
    flatten_result_any(res.map_err(E::into))
}
pub fn flatten_result_with<C, T, E: Into<anyhow::Error>>(
    context: C,
    res: Result<anyhow::Result<T>, E>,
) -> anyhow::Result<T>
where
    C: fmt::Display,
    Result<anyhow::Result<T>, E>: anyhow::Context<anyhow::Result<T>, E>,
{
    let res = res.with_context(move || context.to_string());
    flatten_result_any(res)
}

pub struct RemoteContext {
    pub handle: Handle,
    local_set: Option<ptr::NonNull<LocalSet>>,
}

impl RemoteContext {
    pub const fn new(handle: Handle) -> Self {
        Self { handle, local_set: None }
    }

    pub fn render_local_set(&mut self) -> Option<&mut Option<Box<LocalSet>>> {
        if RenderState::is_render_thread() {
            Some(unsafe { self.render_local_set_unchecked() })
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

/// an Instant calibrated against wall clock time
#[derive(Copy, Clone)]
pub struct WallInstant {
    pub instant: Instant,
    pub timestamp: Timestamp,
}
impl WallInstant {
    /// construct via [Self::calibrated]
    pub fn from_system_time(time: &SystemTime) -> Self {
        Self::from_timestamp(Timestamp::from_system_time(time))
    }
    /// construct via [Self::calibrated]
    pub fn from_instant(instant: StdInstant) -> Self {
        Self::from_tokio_instant(instant)
    }
    /// construct via [Self::calibrated]
    pub fn from_tokio_instant<I: Into<Instant>>(instant: I) -> Self {
        let instant = instant.into();
        let timestamp = Self::timestamp_at_tokio_instant(&instant);
        Self::with_parts(instant, timestamp)
    }
    /// construct via [Self::calibrated]
    pub fn from_timestamp(timestamp: Timestamp) -> Self {
        Self::with_parts(Self::instant_at_timestamp(timestamp).into(), timestamp)
    }
    pub fn now() -> Self {
        #[cfg(todo = "unnecessary")]
        {
            // going backwards seems a bit awkward, so try to avoid
            let _ = Self::calibrated();
        }
        Self::from_tokio_instant(Instant::now())
    }
    /// [Self::now] and a bit
    pub fn soon(wait: Duration) -> Self {
        Self::now().add(wait)
    }
    pub fn from_moment<T: Into<Self>, U: Into<Self>>(moment: Either<T, U>) -> Self {
        match moment {
            Either::Left(l) => l.into(),
            Either::Right(r) => r.into(),
        }
    }
    pub fn now_timestamp_mono() -> Timestamp {
        Self::timestamp_at_instant(&StdInstant::now())
    }
    pub fn now_timestamp_system() -> Timestamp {
        Timestamp::from_system_time(&SystemTime::now())
    }
    pub fn now_timestamp_system_checked() -> Timestamp {
        let now = SystemTime::now();
        let ts = Timestamp::from_system_time(&SystemTime::now());
        let calib_delta = Self::get_calibrated().map(|(sys, calib)| {
            let instant = StdInstant::now();
            let mono_delta = Timestamp::instant_checked_duration_since(&instant, calib.instant.into_std());
            let sys_delta = now.duration_since(sys).map_err(|e| e.duration());
            Timestamp::signed_duration_saturating_sub(sys_delta, mono_delta)
        });
        let recalib = match calib_delta {
            None => false,
            Some(Err(amt)) if amt > Self::DRIFT_THRESHOLD_SYSTEM => {
                log::warn!("system clock drifted behind by {}s, recalibrating", amt.as_secs());
                true
            },
            Some(Ok(amt)) if amt > Self::DRIFT_THRESHOLD_MONO => {
                log::warn!(
                    "monotonic clock drifted behind by {}s, recalibrating",
                    amt.as_secs()
                );
                true
            },
            _ => false,
        };
        if recalib {
            Self::recalibrate();
        }
        ts
    }

    const FAR_ENOUGH_FUTURE: Duration = Duration::from_secs(Self::FAR_ISH_FUTURE.as_secs() * 32);
    /// a couple months is enough for a single session at least!
    const FAR_ISH_FUTURE: Duration = Duration::from_secs(0x800000);
    pub fn far_future_sys() -> &'static SystemTime {
        static FAR_FUTURE: LazyLock<SystemTime> = LazyLock::new(|| {
            Timestamp::MAX_SYS
                .to_system_time()
                .or_else(|| SystemTime::now().checked_add(WallInstant::FAR_ENOUGH_FUTURE))
                .unwrap_or_else(|| SystemTime::now() + WallInstant::FAR_ISH_FUTURE)
        });
        &FAR_FUTURE
    }

    #[inline]
    pub fn with_parts(instant: Instant, timestamp: Timestamp) -> Self {
        Self { instant, timestamp }
    }
    const DRIFT_THRESHOLD_MONO: Duration = Duration::from_secs(60);
    const DRIFT_THRESHOLD_SYSTEM: Duration = Self::DRIFT_THRESHOLD_MONO;
    pub fn new_calibrated(instant: Instant, calibration: &SystemTime) -> Self {
        let timestamp =
            Timestamp::try_from_system_time(calibration).context("now precedes unix, expect time to break");
        Self {
            instant,
            timestamp: rt::log::error_ok(timestamp).unwrap_or_default(),
        }
    }
    fn calibrated_shared() -> &'static RwLock<Option<(SystemTime, Self)>> {
        static CALIBRATED: RwLock<Option<(SystemTime, WallInstant)>> = RwLock::new(None);
        &CALIBRATED
    }
    fn get_calibrated() -> Option<(SystemTime, Self)> {
        Self::calibrated_shared()
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    pub fn calibrated() -> Self {
        let calib = Self::calibrated_shared()
            .read()
            .ok()
            .and_then(|e| e.map(|(_, calib)| calib.clone()));
        match calib {
            Some(calib) => calib,
            None => Self::recalibrate(),
        }
    }
    pub fn recalibrate() -> Self {
        log::trace!("recalibrating mono clock to system");
        let shared = Self::calibrated_shared();
        shared.clear_poison();
        let calib = Self::now_calibrated();
        if let Ok(mut shared) = shared.write() {
            *shared = Some(calib)
        }
        calib.1
    }
    fn now_calibrated() -> (SystemTime, Self) {
        let now_sys = SystemTime::now();
        let now = StdInstant::now();
        let calib = WallInstant::new_calibrated(now.into(), &now_sys);
        (now_sys, calib)
    }

    /// not necessarily far, just already elapsed
    pub fn past_instant() -> &'static StdInstant {
        static PAST: LazyLock<StdInstant> = LazyLock::new(|| {
            let snapshot = WallInstant::calibrated().instant.into_std();
            snapshot
                .checked_sub(Timestamp::DAY)
                .or_else(|| snapshot.checked_sub(Timestamp::HOUR))
                .unwrap_or(snapshot)
        });
        &PAST
    }
    /// An instant that is expected to be earlier than the present
    ///
    /// see [Self::past_instant()] for a stationary/static target
    pub fn passed_instant() -> StdInstant {
        if let Some((_, calib)) = Self::get_calibrated() {
            calib.instant.into_std()
        } else {
            StdInstant::now()
        }
    }
    pub fn far_future() -> Self {
        Self::with_parts(Self::far_future_instant().into(), Timestamp::MAX_INSTANT)
    }
    pub fn far_future_instant() -> StdInstant {
        static FAR_FUTURE: LazyLock<Option<StdInstant>> = LazyLock::new(|| {
            let snapshot = StdInstant::now();
            let future = snapshot
                .checked_add(WallInstant::FAR_ENOUGH_FUTURE)
                .or_else(|| snapshot.checked_add(WallInstant::FAR_ISH_FUTURE));
            if future.is_none() {
                log::error!("not much time left it seems?");
            }
            future
        });
        FAR_FUTURE.unwrap_or_else(|| {
            let snapshot = StdInstant::now();
            snapshot
                .checked_add(Timestamp::WEEK)
                .or(snapshot.checked_add(Timestamp::DAY))
                .unwrap_or(snapshot)
        })
    }
    pub fn as_system_time(&self) -> SystemTime {
        self.timestamp.into_system_time()
    }

    pub fn timestamp_at_instant(instant: &StdInstant) -> Timestamp {
        let cal = Self::calibrated();
        let diff = Timestamp::instant_checked_duration_since(instant, cal.instant.into_std());
        cal.timestamp.saturating_signed_add(diff)
    }
    pub fn timestamp_at_tokio_instant(instant: &Instant) -> Timestamp {
        Self::timestamp_at_instant(&instant.into_std())
    }
    pub fn instant_at_system_time(time: &SystemTime) -> StdInstant {
        let ts = Timestamp::from_system_time(time);
        Self::instant_at_timestamp(ts)
    }
    pub fn instant_at_timestamp(timestamp: Timestamp) -> StdInstant {
        let cal = Self::calibrated();
        let diff = timestamp.checked_duration_since(&cal.timestamp);
        let neg = Timestamp::signed_duration_neg(&diff);
        Timestamp::signed_duration_instant(diff, &cal.instant.into_std())
            .unwrap_or(Self::saturated_instant(neg))
    }

    fn saturated_instant(neg: bool) -> StdInstant {
        match neg {
            true => Self::past_instant().clone(),
            false => Self::far_future_instant(),
        }
    }
    pub fn add(mut self, amt: Duration) -> Self {
        self.timestamp.saturating_add_mut(amt);
        self.instant = self
            .instant
            .checked_add(amt)
            .unwrap_or_else(|| Self::far_future_instant().into());
        self
    }
    pub fn sub(mut self, amt: Duration) -> Self {
        self.timestamp.saturating_sub_mut(amt);
        self.instant = self
            .instant
            .checked_sub(amt)
            .unwrap_or_else(|| Self::past_instant().clone().into());
        self
    }

    #[inline]
    pub fn to_future(&self) -> time::Sleep {
        time::sleep_until(self.instant.clone())
    }
    #[doc(alias = "far_future_future")]
    #[inline]
    pub fn big_sleep() -> time::Sleep {
        Self::far_future().to_future()
    }
    #[inline]
    pub fn no_sleep() -> time::Sleep {
        time::sleep_until(Self::passed_instant().into())
    }
}
/// [Self::from_system_time]
impl From<SystemTime> for WallInstant {
    #[inline]
    fn from(time: SystemTime) -> Self {
        Self::from_system_time(&time)
    }
}
/// [Self::from_instant]
impl From<StdInstant> for WallInstant {
    #[inline]
    fn from(instant: StdInstant) -> Self {
        Self::from_instant(instant)
    }
}
/// [Self::from_tokio_instant]
impl From<Instant> for WallInstant {
    #[inline]
    fn from(instant: Instant) -> Self {
        Self::from_tokio_instant(instant)
    }
}
/// [Self::from_timestamp]
impl From<Timestamp> for WallInstant {
    #[inline]
    fn from(timestamp: Timestamp) -> Self {
        Self::from_timestamp(timestamp)
    }
}
/// [Self::soon]
impl From<Duration> for WallInstant {
    #[inline]
    fn from(wait: Duration) -> Self {
        Self::soon(wait)
    }
}
impl From<WallInstant> for Timestamp {
    #[inline]
    fn from(i: WallInstant) -> Self {
        i.timestamp
    }
}
impl From<WallInstant> for Instant {
    #[inline]
    fn from(i: WallInstant) -> Self {
        i.instant
    }
}
impl From<WallInstant> for StdInstant {
    #[inline]
    fn from(i: WallInstant) -> Self {
        i.instant.into_std()
    }
}
impl From<WallInstant> for SystemTime {
    #[inline]
    fn from(i: WallInstant) -> Self {
        i.as_system_time()
    }
}
#[test]
fn future_is_now_sys() {
    let now_sys = SystemTime::now();
    let headroom = WallInstant::far_future_sys().duration_since(&now_sys).unwrap();
    assert!(headroom > WallInstant::FAR_ENOUGH_FUTURE);
}
#[test]
fn future_is_now_instant() {
    let now = Instant::now();
    let headroom = WallInstant::far_future_instant().duration_since(&now).unwrap();
    assert!(headroom >= WallInstant::FAR_ENOUGH_FUTURE);
}
