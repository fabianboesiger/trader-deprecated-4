use crate::{Error, Market};
use openlimits::{binance::Binance as OpenLimitsBinance, exchange::ExchangeAccount};
use rust_decimal::prelude::*;
use std::collections::HashMap;
use tokio::sync::Mutex;

type Symbol = String;

#[derive(Debug, Default)]
struct Asset {
    price: Decimal,
    quantity: Decimal,
}

impl Asset {
    fn value(&self) -> Decimal {
        self.quantity * self.price
    }
}

#[derive(Debug)]
pub struct Wallet(Mutex<HashMap<Symbol, Asset>>);

impl Wallet {
    pub const QUOTE_ASSET: &'static str = "USDT";
    pub const FEE_ASSET: &'static str = "BNB";

    pub fn new() -> Self {
        let mut map = HashMap::new();
        map.insert(
            Wallet::QUOTE_ASSET.to_owned(),
            Asset {
                price: Decimal::one(),
                quantity: Decimal::zero(),
            },
        );

        Wallet(Mutex::new(map))
    }

    pub async fn update(&self, exchange: &OpenLimitsBinance) -> Result<(), Error> {
        let balances = exchange.get_account_balances(None).await?;

        let mut wallet = self.0.lock().await;
        for balance in balances {
            wallet.entry(balance.asset).or_default().quantity = balance.total;
        }

        Ok(())
    }

    pub async fn update_price(&self, mut market: Market, price: Decimal) {
        let usdt_offset = market.find(Wallet::QUOTE_ASSET).unwrap();
        self.0
            .lock()
            .await
            .entry(market.drain(..usdt_offset).collect())
            .or_default()
            .price = price;
    }

    pub async fn update_quantity(&self, asset: Symbol, quantity: Decimal) {
        self.0.lock().await.entry(asset).or_default().quantity = quantity;
    }

    pub async fn value<A: AsRef<str>>(&self, asset: A) -> Decimal {
        self.0
            .lock()
            .await
            .get(asset.as_ref())
            .map(|position| position.value())
            .unwrap_or(Decimal::zero())
    }

    pub async fn total_value(&self) -> Decimal {
        self.0.lock().await.values().map(Asset::value).sum()
    }
    /*
    async fn update_positions(&self, exchange: &OpenLimitsBinance) -> Decimal {
        let guard = self.0.lock().await;

    }*/
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    #[tokio::test]
    async fn wallet() {
        let wallet = Wallet::new();
        assert_eq!(wallet.total_value().await, Decimal::new(0, 0));
        wallet
            .update_quantity("BTC".to_owned(), Decimal::new(1, 0))
            .await;
        wallet
            .update_price("BTCUSDT".to_owned(), Decimal::new(50000, 0))
            .await;
        assert_eq!(wallet.total_value().await, Decimal::new(50000, 0));
        wallet
            .update_quantity(Wallet::QUOTE_ASSET.to_owned(), Decimal::new(10000, 0))
            .await;
        assert_eq!(wallet.total_value().await, Decimal::new(60000, 0));
    }
}
