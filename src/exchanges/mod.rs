mod binance;
mod historical;

pub use binance::Binance;
pub use historical::Historical;

use crate::{Market, Number, Strategy};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[async_trait]
pub trait Exchange<S: Strategy>: Send + 'static {
    async fn run(self, strategy: &mut S);
}

#[derive(Debug, Clone)]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Clone)]
pub struct Order {
    pub market: Market,
    pub price: Number,
    pub take_profit: Option<Number>,
    pub stop_loss: Option<Number>,
    pub side: Side,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub market: Market,
    pub quantity: Number,
    pub price: Number,
    pub timestamp: i64,
}
