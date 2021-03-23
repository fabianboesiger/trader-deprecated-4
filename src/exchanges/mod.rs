mod binance;
mod historical;

pub use binance::Binance;
pub use historical::Historical;

use crate::{Market, Number, Strategy};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;

#[async_trait]
pub trait Exchange<S: Strategy>: Send + 'static {
    async fn run(self, strategy: &mut S);
}

#[derive(Debug, Clone)]
pub enum Side {
    Buy,
    Sell,
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Buy => "buy",
                Self::Sell => "sell",
            }
        )
    }
}

#[derive(Debug, Clone)]
pub struct Order {
    pub market: Market,
    pub price: Number,
    pub take_profit: Option<Number>,
    pub stop_loss: Option<Number>,
    pub side: Side,
}

impl fmt::Display for Order {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} {}{}{}",
            self.side,
            self.price,
            self.market,
            if let Some(take_profit) = self.take_profit {
                format!(", take profit: {}", take_profit)
            } else {
                String::new()
            },
            if let Some(stop_loss) = self.stop_loss {
                format!(", stop loss: {}", stop_loss)
            } else {
                String::new()
            }
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub market: Market,
    pub quantity: Number,
    pub price: Number,
    pub timestamp: i64,
}
