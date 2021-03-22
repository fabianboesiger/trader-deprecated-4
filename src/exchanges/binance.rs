use super::{Exchange, Strategy, Trade, Order};
use crate::Market;
use async_trait::async_trait;
use futures::stream::StreamExt;
use openlimits::{
    binance::{
        Binance as OpenLimitsBinance, BinanceCredentials, BinanceParameters, BinanceWebsocket,
    },
    exchange::{Exchange as OpenLimitsExchange, ExchangeAccount},
    exchange_info::ExchangeInfoRetrieval,
    exchange_ws::{ExchangeWs, OpenLimitsWs},
    model::{
        websocket::{OpenLimitsWebSocketMessage, Subscription, WebSocketResponse},
        Side,
    },
    model::{Balance, OpenLimitOrderRequest, TimeInForce},
};
use rust_decimal::prelude::*;
use sqlx::postgres::PgPool;
use std::collections::HashMap;
use tokio::sync::mpsc;

enum Status {
    Buy,
    Hold,
    Sell,
}

struct Position {
    status: Status
}

pub struct Binance {
    sandbox: bool,
    positions: HashMap<Market, Position>,
    markets: Vec<Market>,
    exchange: OpenLimitsBinance,
}

impl Binance {
    pub async fn new(sandbox: bool) -> Self {
        let exchange = OpenLimitsBinance::new(BinanceParameters {
                sandbox,
                credentials: Some(BinanceCredentials {
                    api_key: std::env::var("BINANCE_API_KEY").expect("Couldn't get BINANCE_API_KEY."),
                    api_secret: std::env::var("BINANCE_API_SECRET")
                        .expect("Couldn't get BINANCE_API_SECRET."),
                }),
            })
            .await
            .expect("Failed to create Client");

        Self {
            sandbox,
            positions: HashMap::new(),
            markets: vec![
                "BTCUSDT",
                "ETHUSDT",
                "CHZUSDT",
                "BNBUSDT",
                "DOGEUSDT",
                "ADAUSDT",
                "BCHUSDT",
                "XRPUSDT",
                "LTCUSDT",
                "EOSUSDT",
                "DOTUSDT"
            ]
                .into_iter()
                .map(String::from)
                .collect(),
            exchange,
        }
    }
}

#[async_trait]
impl<S: Strategy + 'static> Exchange<S> for Binance {
    async fn run(mut self, strategy: &mut S) {
        dotenv::dotenv().ok();

        let (tx, mut rx) = mpsc::unbounded_channel();
        let sandbox = self.sandbox;

        /*
        let exchange = exchange.inner_client().unwrap();

        println!(
            "{:#?}",
            exchange
                .get_exchange_info()
                .await
                .expect("Couldn't get exchange info.")
        );
        */
        //println!("{:?}", self.exchange.get_pair("BTCUSDT").await.unwrap());

        let subscriptions = self.markets
            .iter()
            .map(|symbol| Subscription::Trades(symbol.to_lowercase().to_string()))
            .collect::<Vec<Subscription>>();

        tokio::join!(
            async move {
                loop {
                    let mut stream = OpenLimitsWs {
                        websocket: BinanceWebsocket::new(if sandbox {
                            BinanceParameters::sandbox()
                        } else {
                            BinanceParameters::prod()
                        })
                        .await
                        .expect("Failed to create Client"),
                    }
                    .create_stream(&subscriptions)
                    .await
                    .expect("Couldn't create stream.");

                    while let Some(Ok(message)) = stream.next().await {
                        match message {
                            WebSocketResponse::Generic(OpenLimitsWebSocketMessage::Trades(
                                trades,
                            )) => {
                                for trade in trades {
                                    let market = trade.market_pair;
                                    let quantity = match trade.side {
                                        Side::Buy => -trade.qty,
                                        Side::Sell => trade.qty,
                                    };
                                    let price = trade.price;
                                    let timestamp = trade.created_at as i64;

                                    let trade = Trade {
                                        market,
                                        quantity: quantity.to_f32().unwrap(),
                                        price: price.to_f32().unwrap(),
                                        timestamp,
                                    };

                                    tx.send(trade).unwrap();
                                }
                            }
                            _ => (),
                        }
                    }
                }
            },
            async move {
                while let Some(trade) = rx.recv().await {
                    if let Some(order) = strategy.run(trade) {
                        self.order(order).await.ok();
                    }
                }
            }
        );
    }
}

impl Binance {
    async fn order(&mut self, order: Order) -> Result<(), Box<dyn std::error::Error>> {
        let price = Decimal::from_f32(order.price).unwrap();
        let take_profit = Decimal::from_f32(order.take_profit.unwrap()).unwrap();
        let stop_loss = Decimal::from_f32(order.stop_loss.unwrap()).unwrap();

        let balance = self.exchange
            .get_account_balances(None)
            .await?
            .into_iter()
            .filter(|balance| balance.asset == "USDT")
            .map(|balance| balance.free)
            .next()
            .unwrap_or(Decimal::zero());

        // TODO: Remove
        let balance = Decimal::from_f32(20.0).unwrap();
        
        if balance > Decimal::zero() {
            let buy_order = self.exchange.limit_buy(&OpenLimitOrderRequest {
                market_pair: order.market.clone(),
                size: balance / price,
                price,
                time_in_force: TimeInForce::FillOrKill,
                post_only: false,
            }).await?;

            println!("{:?}", buy_order);

            debug_assert_eq!(buy_order.status, openlimits::model::OrderStatus::Expired);

            let pair = self.exchange.get_pair(&order.market).await?.read()?;
            let incr = pair.quote_increment;

            let inner = self.exchange.inner_client().unwrap();
            
            let sell_order = inner.oco_sell(
                pair,
                buy_order.size,
                take_profit,
                stop_loss,
                Some(stop_loss - incr),
                Some(TimeInForce::GoodTillCancelled.into()),
            ).await?;

            println!("{:?}", sell_order);
        }

        Ok(())
    }
}
