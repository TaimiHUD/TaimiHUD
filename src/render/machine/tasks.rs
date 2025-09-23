use {
    crate::render::{
        machine::RenderMachine,
        RenderState,
    },
    std::collections::VecDeque,
    tokio::sync::Mutex,
};

pub type RenderTaskQueue = VecDeque<RenderTask>;
pub type RenderTask = Box<dyn FnOnce(&mut RenderState) + Send + 'static>;

impl RenderMachine {
    pub fn shared_task_queue() -> &'static Mutex<RenderTaskQueue> {
        static TASK_QUEUE: Mutex<RenderTaskQueue> = Mutex::const_new(VecDeque::new());

        &TASK_QUEUE
    }

    pub fn schedule_task_blocking(task: RenderTask, prio: RenderTaskPriority) {
        match prio {
            #[cfg(todo)]
            RenderTaskPriority::Immediate => {
                tokio::task::spawn_blocking(move ||
                    crate::RenderState::lock().task_queue.push_back(task)
                ).await
            },
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
            RenderTaskPriority::Immediate => {
                tokio::task::spawn_blocking(move ||
                    crate::RenderState::lock().task_queue.push_back(task)
                ).await
            },
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

        let Some((task, remaining)) = Self::try_pop_task() else {
            return
        };

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
}

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RenderTaskPriority {
    #[default]
    Normal,
    High,
    Immediate,
}
