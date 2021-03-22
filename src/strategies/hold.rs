use super::Strategy;
use crate::exchanges::{Order, Side, Trade};
use std::fmt;

#[derive(Copy, Clone)]
pub struct Hold {}

impl Hold {
    pub fn new() -> Self {
        Self {}
    }
}

impl Strategy for Hold {
    fn run(&mut self, Trade { market, price, .. }: Trade) -> Option<Order> {
        Some(Order {
            market,
            price,
            take_profit: None,
            stop_loss: None,
            side: Side::Buy,
        })
    }
}

impl fmt::Display for Hold {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "hold")
    }
}
