use {
    anyhow::Context,
    crate::{
        controller::Controller,
        render::{
            machine::{RenderMachine, RenderTask, RenderTaskPriority},
            RenderState,
        },
    },
    std::time::Duration,
    tokio::{
        runtime::{Builder, Handle, Runtime},
        sync::oneshot,
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
}
