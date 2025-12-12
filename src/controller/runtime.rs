use {
    crate::{
        controller::Controller,
        render::{
            machine::{RenderMachine, RenderTask, RenderTaskPriority},
            RenderState,
        },
    },
    anyhow::Context,
    std::{ptr, time::Duration},
    tokio::{
        runtime::{Builder, Handle, Runtime},
        sync::oneshot,
        task::LocalSet,
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
