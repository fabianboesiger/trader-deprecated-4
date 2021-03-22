use super::{Order, Strategy, Trade};
use async_trait::async_trait;
use std::fmt;

#[derive(Clone)]
pub struct Interval<S: Strategy + Clone> {
    strategy: S,
    interval: i64,
    trade: Option<Trade>,
}

impl<S: Strategy + Clone> Interval<S> {
    pub fn new(strategy: S, interval: i64) -> Self {
        Self {
            strategy,
            interval,
            trade: None,
        }
    }
}

#[async_trait]
impl<S: Strategy + Clone> Strategy for Interval<S> {
    fn run(&mut self, trade: Trade) -> Option<Order> {
        if let Some(old) = &mut self.trade {
            if old.timestamp / self.interval != trade.timestamp / self.interval {
                let output = old.clone();
                *old = trade;
                self.strategy.run(output)
            } else {
                old.timestamp = trade.timestamp;
                old.price = trade.price;
                old.quantity += trade.quantity;
                None
            }
        } else {
            self.trade = Some(trade);
            None
        }
    }
}

impl<S: Strategy + Clone> fmt::Display for Interval<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.strategy)
    }
}
