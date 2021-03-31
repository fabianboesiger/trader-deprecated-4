use super::Strategy;
use crate::{
    exchanges::{Order, Side, Trade},
    indicators::*,
};
use std::fmt;

#[derive(Copy, Clone)]
pub struct Custom {
    val: Val,
    diff: Ema,
    diff_stdev: Stdev,
    macd: Macd,
    stdev: Stdev,
    was_undervalued: bool,
}

impl Custom {
    pub fn new() -> Self {
        Self {
            val: Val::new(200.0, 2000.0),
            diff: Ema::new(30.0),
            diff_stdev: Stdev::new(200.0),
            macd: Macd::new(10.0, 20.0, 5.0),
            stdev: Stdev::new(200.0),
            was_undervalued: false,
        }
    }
}

impl Strategy for Custom {
    fn run(
        &mut self,
        Trade {
            market,
            quantity,
            price,
            ..
        }: Trade,
    ) -> Option<Order> {
        log::trace!("Running strategy.");

        self.stdev.run(price);
        self.val.run(quantity, price);
        self.diff.run(price - self.val.get());
        self.diff_stdev.run(self.diff.get());
        self.macd.run(price);

        let is_undervalued = self.diff.get() < -self.diff_stdev.get() * 1.2;
        let worth_it = 1.0 * self.stdev.get() > price * 0.01;
        let has_momentum = self.macd.get_hist() > 0.0;

        let action = if !is_undervalued && self.was_undervalued && worth_it && has_momentum {
            Some(Order {
                market,
                price,
                take_profit: Some(price + 1.2 * self.stdev.get()),
                stop_loss: Some(price - 1.0 * self.stdev.get()),
                side: Side::Buy,
            })
        } else {
            None
        };

        self.was_undervalued = is_undervalued;

        action
    }
}

impl fmt::Display for Custom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "custom")
    }
}
