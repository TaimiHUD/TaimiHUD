pub use {
    core::time::Duration as TimeSpan,
    glam::{Vec2, Vec3A as Vec3},
};

use crate::script::pathing;

pub type Colour = u32;
pub type TickSpan = u32;

pub struct GameTime {
    pub elapsed: TimeSpan,
    pub elapsed_ticks: TickSpan,
    pub total: TimeSpan,
    pub total_ticks: TickSpan,
}
impl pathing::GameTime for GameTime {
    type TimeSpan = TimeSpan;
    type TickSpan = TickSpan;

    fn elapsed_game_time(&self) -> Self::TimeSpan {
        self.elapsed
    }
    fn elapsed_game_ticks(&self) -> Self::TickSpan {
        self.elapsed_ticks
    }
    fn total_game_time(&self) -> Self::TimeSpan {
        self.total
    }
    fn total_game_ticks(&self) -> Self::TickSpan {
        self.total_ticks
    }
}

pub type Size2U = [u32; 2];
