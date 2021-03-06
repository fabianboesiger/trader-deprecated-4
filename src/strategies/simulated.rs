use super::{Order, Strategy, Trade};
use crate::Number;
use async_trait::async_trait;
use chrono::NaiveDateTime;
use std::fmt;

fn format_timestamp(timestamp: i64) -> String {
    let date_time = NaiveDateTime::from_timestamp(timestamp / 1000, 0);
    format!("{}", date_time.format("%c"))
}

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
    consecutive_losses: u8,
    wait_until: i64,
}

impl<S: Strategy> Simulated<S> {
    pub fn new(strategy: S, fee: f32, concurrency: usize) -> Self {
        Self {
            strategy,
            open: Vec::new(),
            closed: Vec::new(),
            concurrency,
            fee,
            consecutive_losses: 0,
            wait_until: 0,
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

            if self.open.len() < self.concurrency && !already_open && self.wait_until < timestamp {
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

        for order in &closed {
            if order.buy_price > order.sell_price {
                self.consecutive_losses += 1;
                if self.consecutive_losses >= 2 {
                    self.consecutive_losses = 0;
                    self.wait_until = timestamp + 1000 * 60 * 60 * 24;
                }
            } else {
                self.consecutive_losses = 0;
            }
        }

        self.open.append(&mut open);
        self.closed.append(&mut closed);

        None
    }

    #[cfg(feature = "plot")]
    fn plot(&self) {
        self.strategy.plot()
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
                "{}:\t {:+.2}%\t (CLOSED)\t {}\t - {}",
                order.market,
                (sell_price / buy_price * (1.0 - 2.0 * self.fee) - 1.0) * 100.0,
                format_timestamp(*buy_time),
                format_timestamp(*sell_time),
            )?;
        }
        for OrderHistory {
            order,
            buy_price,
            sell_price,
            buy_time,
            ..
        } in &self.open
        {
            writeln!(
                f,
                "{}:\t {:+.2}%\t (OPEN)\t {}",
                order.market,
                (sell_price / buy_price * (1.0 - 2.0 * self.fee) - 1.0) * 100.0,
                format_timestamp(*buy_time),
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

        let wins = self
            .closed
            .iter()
            .filter(
                |OrderHistory {
                     buy_price,
                     sell_price,
                     ..
                 }| buy_price < sell_price,
            )
            .count();

        let losses = self
            .closed
            .iter()
            .filter(
                |OrderHistory {
                     buy_price,
                     sell_price,
                     ..
                 }| buy_price >= sell_price,
            )
            .count();

        writeln!(
            f,
            "TOTAL:  \t {:+.2}%\n{:+.2}% per trade\n{} trades ({:.2}% profitable)",
            total,
            total / (self.open.len() + self.closed.len()) as f32,
            wins + losses,
            wins as f32 / (wins + losses) as f32 * 100.0,
        )?;

        Ok(())
    }
}
