use {
    anyhow::Context,
    crate::controller::Controller,
    tokio::runtime::{Builder, Handle, Runtime},
    std::time::Duration,
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
}
