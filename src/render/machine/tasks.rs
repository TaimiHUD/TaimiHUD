use {
    crate::render::{machine::RenderMachine, RenderState},
    std::{cell::RefCell, collections::VecDeque},
    tokio::sync::Mutex,
};

#[cfg(feature = "space")]
use crate::space::Engine;

pub type RenderTaskQueue = VecDeque<RenderTask>;
pub type RenderTask = Box<dyn FnOnce(&mut RenderState) + Send + 'static>;

impl RenderMachine {
    /// TODO: can't this be a std mutex? likely not held for any amount of time,
    /// let alone across awaits...
    pub fn shared_task_queue() -> &'static Mutex<RenderTaskQueue> {
        static TASK_QUEUE: Mutex<RenderTaskQueue> = Mutex::const_new(VecDeque::new());

        &TASK_QUEUE
    }

    pub fn schedule_task_blocking(task: RenderTask, prio: RenderTaskPriority) {
        match prio {
            #[cfg(todo)]
            RenderTaskPriority::Immediate =>
                tokio::task::spawn_blocking(move || crate::RenderState::lock().task_queue.push_back(task))
                    .await,
            prio => {
                let mut queue = Self::shared_task_queue().blocking_lock();
                match prio {
                    RenderTaskPriority::Normal => queue.push_back(task),
                    _ => queue.push_front(task),
                }
            },
        }
    }

    pub async fn schedule_task_async(task: RenderTask, prio: RenderTaskPriority) {
        match prio {
            #[cfg(todo)]
            RenderTaskPriority::Immediate =>
                tokio::task::spawn_blocking(move || crate::RenderState::lock().task_queue.push_back(task))
                    .await,
            prio => {
                let mut queue = Self::shared_task_queue().lock().await;
                match prio {
                    RenderTaskPriority::Normal => queue.push_back(task),
                    _ => queue.push_front(task),
                }
            },
        }
    }

    pub(crate) fn try_pop_task() -> Option<(RenderTask, usize)> {
        let mut queue = Self::shared_task_queue().try_lock().ok()?;

        let task = queue.pop_front();
        task.map(|task| {
            let remaining = queue.len();
            (task, remaining)
        })
    }

    pub(crate) fn pop_task() -> Option<RenderTask> {
        Self::shared_task_queue().blocking_lock().pop_front()
        // TODO: or Self::try_pop_task().map(|(task, _)| task)?
    }

    const TASK_CATCH_UP_THRESHOLD: usize = 16;

    pub(crate) fn run_tasks(state: &mut RenderState) {
        while let Some(task) = state.task_queue.pop_front() {
            task(state);
        }

        let Some((task, remaining)) = Self::try_pop_task() else { return };

        task(state);

        let catch_up = remaining.saturating_sub(Self::TASK_CATCH_UP_THRESHOLD);
        if catch_up > 0 {
            for _ in 0..=catch_up / 3 {
                if let Some(task) = Self::pop_task() {
                    task(state);
                }
            }
        }
    }

    pub fn with_runtime_machine_ref<R, F: FnOnce(&RenderMachine) -> R>(f: F) -> R {
        Self::runtime_machine().with(|m| f(&m.borrow()))
    }
    pub fn with_runtime_machine<R, F: FnOnce(&mut RenderMachine) -> R>(f: F) -> R {
        Self::runtime_machine().with(|m| f(&mut m.borrow_mut()))
    }
    #[cfg(feature = "space")]
    pub fn with_runtime_engine_ref<R, F: FnOnce(Option<&Engine>) -> R>(f: F) -> R {
        Self::runtime_engine().with(|m| f(m.borrow().as_ref().map(|e| &**e)))
    }
    #[cfg(feature = "space")]
    pub fn with_runtime_engine<R, F: FnOnce(Option<&mut Engine>) -> R>(f: F) -> R {
        Self::runtime_engine().with(|m| f(m.borrow_mut().as_mut().map(|e| &mut **e)))
    }

    fn runtime_machine() -> &'static tokio::task::LocalKey<RefCell<&'static mut RenderMachine>> {
        tokio::task_local! {
            static LOCAL_MACHINE: RefCell<&'static mut RenderMachine>;
        }
        &LOCAL_MACHINE
    }
    #[cfg(feature = "space")]
    fn runtime_engine() -> &'static tokio::task::LocalKey<RefCell<Option<&'static mut Engine>>> {
        tokio::task_local! {
            static LOCAL_ENGINE: RefCell<Option<&'static mut Engine>>;
        }
        &LOCAL_ENGINE
    }

    const POLL_ITERATION_LIMIT: usize = 64;
    unsafe fn pretend_leak<'i, 'o, T: ?Sized>(i: &'i mut T) -> &'o mut T {
        core::mem::transmute(i)
    }
    pub(crate) fn poll_runtime(state: &mut RenderState) {
        if let Some(runtime) = &mut state.runtime {
            use taimi_sync::poll_once::PollOnce;

            // TODO: return both set and lifetime or split struct up
            let handle = runtime.handle.clone();
            let locals = match runtime.render_local_set() {
                Some(Some(locals)) => locals,
                Some(locals @ &mut None) => locals.insert(Box::new(tokio::task::LocalSet::new())),
                None => {
                    log::error!("polling render runtime outside of render thread???");
                    return
                },
            };

            let locals = Self::runtime_machine().scope(
                RefCell::new(unsafe { Self::pretend_leak(&mut state.machine) }),
                locals,
            );
            #[cfg(feature = "space")]
            let locals = {
                let engine = state
                    .engine
                    .as_mut()
                    .and_then(|e| e.as_mut().ok())
                    .map(|e| unsafe { Self::pretend_leak(e) });
                Self::runtime_engine().scope(RefCell::new(engine), locals)
            };
            let locals_once = PollOnce::new(locals);
            tokio::pin!(locals_once);
            let res = handle.block_on(locals_once.with_limit(Self::POLL_ITERATION_LIMIT));
            match res {
                // waiting on IO or something, move on
                None => (),
                // no work left to do!
                Some(()) => (),
            }
        }
    }
}

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RenderTaskPriority {
    #[default]
    Normal,
    High,
    Immediate,
}
