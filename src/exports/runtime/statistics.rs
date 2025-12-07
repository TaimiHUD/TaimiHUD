#[cfg(feature = "statistics")]
pub use taimi_hoard::statistics::Counter;
#[cfg(not(feature = "statistics"))]
pub use taimi_hoard::statistics::Dummy as Counter;
