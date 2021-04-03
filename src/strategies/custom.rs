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
    hist_stdev: Stdev,
    macd: Macd,
    stdev: Stdev,
    was_undervalued: bool,
    bought_at: i64,
}

impl Custom {
    pub fn new() -> Self {
        Self {
            val: Val::new(200.0, 2000.0),
            diff: Ema::new(30.0),
            diff_stdev: Stdev::new(200.0),
            hist_stdev: Stdev::new(200.0),
            macd: Macd::new(10.0, 20.0, 5.0),
            stdev: Stdev::new(200.0),
            was_undervalued: false,
            bought_at: 0,
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
            timestamp,
        }: Trade,
    ) -> Option<Order> {
        log::trace!("Running strategy.");

        self.stdev.run(price);
        self.val.run(quantity, price);
        self.diff.run(price - self.val.get());
        self.diff_stdev.run(self.diff.get());
        self.macd.run(price);
        self.hist_stdev.run(self.macd.get_hist());

        let is_undervalued = self.diff.get() < -self.diff_stdev.get() * 1.2;
        let worth_it = 1.0 * self.stdev.get() > price * 0.01;
        let has_momentum =
            self.macd.get_hist() > 0.0 && self.macd.get_hist() < 1.0 * self.hist_stdev.get();

        let action = if
            !is_undervalued &&
            self.was_undervalued &&
            worth_it &&
            has_momentum && 
            self.bought_at + 1000 * 60 * 60 * 24 < timestamp
        {
            self.bought_at = timestamp;
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
