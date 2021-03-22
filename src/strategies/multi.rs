use super::{Order, Strategy, Trade};
use async_trait::async_trait;
use std::fmt;

pub struct Multi {
    strategies: Vec<Box<dyn Strategy>>,
}

impl Multi {
    pub fn new() -> Self {
        Self {
            strategies: Vec::new(),
        }
    }

    pub fn with<S: Strategy>(mut self, strategy: S) -> Self {
        self.strategies.push(Box::new(strategy));
        self
    }
}

#[async_trait]
impl Strategy for Multi {
    fn run(&mut self, trade: Trade) -> Option<Order> {
        for strategy in &mut self.strategies {
            strategy.run(trade.clone());
        }

        None
    }
}

impl fmt::Display for Multi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for strategy in &self.strategies {
            writeln!(f, "{}", strategy)?;
        }

        Ok(())
    }
}
