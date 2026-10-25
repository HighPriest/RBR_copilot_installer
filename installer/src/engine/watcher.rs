/// What the engine is doing, for the progress screen.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Checking,
    Uninstalling,
    BackingUp,
    Installing,
}

/// Where a run is, and what happened, for the progress bar and the log.
pub trait Watcher {
    /// `done`/`total` are bytes across the whole phase, not per package.
    fn phase(&mut self, phase: Phase, item: &str, done: u64, total: u64);
    fn log(&mut self, line: &str);
}

/// A watcher that ignores everything.
pub struct Silent;

impl Watcher for Silent {
    fn phase(&mut self, _: Phase, _: &str, _: u64, _: u64) {}
    fn log(&mut self, _: &str) {}
}
