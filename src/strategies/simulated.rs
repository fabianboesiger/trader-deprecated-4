use super::{Order, Strategy, Trade};
use crate::Number;
use async_trait::async_trait;
use std::fmt;

struct OrderHistory {
    order: Order,
    buy_price: Number,
    buy_time: i64,
    sell_price: Number,
    sell_time: i64,
}

pub struct Simulated<S> {
    strategy: S,
    open: Vec<OrderHistory>,
    closed: Vec<OrderHistory>,
    concurrency: usize,
    fee: f32,
}

impl<S: Strategy> Simulated<S> {
    pub fn new(strategy: S, fee: f32, concurrency: usize) -> Self {
        Self {
            strategy,
            open: Vec::new(),
            closed: Vec::new(),
            concurrency,
            fee,
        }
    }
}

#[async_trait]
impl<S: Strategy> Strategy for Simulated<S> {
    fn run(&mut self, trade: Trade) -> Option<Order> {
        let price = trade.price;
        let market = trade.market.clone();
        let timestamp = trade.timestamp;
        let mut already_open = false;

        for OrderHistory {
            sell_price,
            sell_time,
            ..
        } in self
            .open
            .iter_mut()
            .filter(|OrderHistory { order, .. }| order.market == market)
        {
            *sell_price = price;
            *sell_time = trade.timestamp;
            already_open = true;
        }

        if let Some(order) = self.strategy.run(trade) {
            assert_eq!(order.market, market);
            // TODO: Check if price is in range.

            if self.open.len() < self.concurrency && !already_open {
                self.open.push(OrderHistory {
                    order,
                    buy_price: price,
                    buy_time: timestamp,
                    sell_price: price,
                    sell_time: timestamp,
                });
            }
        }

        let (mut closed, mut open): (Vec<OrderHistory>, Vec<OrderHistory>) =
            self.open.drain(..).partition(|OrderHistory { order, .. }| {
                order.market == market
                    && (order
                        .take_profit
                        .map_or(false, |take_profit| price >= take_profit)
                        || order
                            .stop_loss
                            .map_or(false, |stop_loss| price <= stop_loss))
            });

        self.open.append(&mut open);
        self.closed.append(&mut closed);

        None
    }
}

impl<S: Strategy> fmt::Display for Simulated<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "type: simulation\nfees: {}%\nfraction of investment per trade: 1/{}\nstrategy: {}",
            self.fee * 100.0,
            self.concurrency,
            self.strategy
        )?;

        for OrderHistory {
            order,
            buy_price,
            buy_time,
            sell_price,
            sell_time,
        } in &self.closed
        {
            writeln!(
                f,
                "{}:\t {:+.2}%\t (CLOSED)\t {} min",
                order.market,
                (sell_price / buy_price * (1.0 - 2.0 * self.fee) - 1.0) * 100.0,
                (sell_time - buy_time) / 1000 / 60
            )?;
        }
        for OrderHistory {
            order,
            buy_price,
            sell_price,
            ..
        } in &self.open
        {
            writeln!(
                f,
                "{}:\t {:+.2}%\t (OPEN)",
                order.market,
                (sell_price / buy_price * (1.0 - 2.0 * self.fee) - 1.0) * 100.0
            )?;
        }

        let total = self
            .closed
            .iter()
            .chain(self.open.iter())
            .map(
                |OrderHistory {
                     buy_price,
                     sell_price,
                     ..
                 }| (sell_price / buy_price * (1.0 - 2.0 * self.fee) - 1.0),
            )
            .sum::<f32>()
            / self.concurrency as f32
            * 100.0;

        writeln!(
            f,
            "TOTAL:  \t {:+.2}%\t ({:+.2}% per trade)",
            total,
            total / (self.open.len() + self.closed.len()) as f32,
        )?;

        Ok(())
    }
}
