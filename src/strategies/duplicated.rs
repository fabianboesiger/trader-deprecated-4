use super::{Order, Strategy, Trade};
use crate::Market;
use async_trait::async_trait;
use std::collections::HashMap;
use std::fmt;

pub struct Duplicated<S: Strategy + Clone> {
    strategy: S,
    strategies: HashMap<Market, S>,
}

impl<S: Strategy + Clone> Duplicated<S> {
    pub fn new(strategy: S) -> Self {
        Self {
            strategy,
            strategies: HashMap::new(),
        }
    }
}

#[async_trait]
impl<S: Strategy + Clone> Strategy for Duplicated<S> {
    fn run(&mut self, trade: Trade) -> Option<Order> {
        self.strategies
            .entry(trade.market.clone())
            .or_insert(self.strategy.clone())
            .run(trade)
    }

    fn plot(&self) {
        for strategy in self.strategies.values() {
            strategy.plot();
        }
    }
}

impl<S: Strategy + Clone> fmt::Display for Duplicated<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.strategy)
    }
}
