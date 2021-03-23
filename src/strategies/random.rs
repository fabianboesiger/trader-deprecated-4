use super::Strategy;
use crate::{
    exchanges::{Order, Side, Trade},
    indicators::*,
};
use std::fmt;

#[derive(Clone)]
pub struct Random {
    stdev: Stdev,
}

impl Random {
    pub fn new() -> Self {
        Self {
            stdev: Stdev::new(200.0),
        }
    }
}

impl Strategy for Random {
    fn run(&mut self, Trade { market, price, .. }: Trade) -> Option<Order> {
        self.stdev.run(price);

        if rand::random::<f32>() < 0.1f32 {
            Some(Order {
                market,
                price,
                take_profit: Some(price + 1.0 * self.stdev.get()),
                stop_loss: Some(price - 1.0 * self.stdev.get()),
                side: Side::Buy,
            })
        } else {
            None
        }
    }
}

impl fmt::Display for Random {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "random")
    }
}
