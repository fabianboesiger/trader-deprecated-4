mod custom;
mod duplicated;
mod hold;
mod interval;
mod multi;
mod simulated;

pub use custom::Custom;
pub use duplicated::Duplicated;
pub use hold::Hold;
pub use interval::Interval;
pub use multi::Multi;
pub use simulated::Simulated;

use crate::{Order, Trade};
use std::fmt::Display;

pub trait Strategy: Display + Send + 'static {
    fn run(&mut self, trade: Trade) -> Option<Order>;
}
