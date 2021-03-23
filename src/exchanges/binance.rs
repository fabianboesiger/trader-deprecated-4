use super::{Exchange, Order, Strategy, Trade};
use crate::Market;
use async_trait::async_trait;
use futures::stream::{BoxStream, StreamExt};
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
    shared::Result as OpenLimitsResult,
};
use rust_decimal::prelude::*;
use std::collections::HashMap;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::time::{sleep, timeout, Duration};

enum Status {
    Buy,
    Hold,
    Sell,
}

struct Position {
    status: Status,
}

pub struct Binance {
    sandbox: bool,
    positions: HashMap<Market, Position>,
    markets: Vec<Market>,
    exchange: OpenLimitsBinance,
}

impl Binance {
    pub async fn new(markets: Vec<&str>, sandbox: bool) -> Self {
        log::info!("Connecting to exchange.");

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

        log::info!("Getting exchange info.");

        let inner = exchange.inner_client().unwrap();
        let info = inner.get_exchange_info().await;

        Self {
            sandbox,
            positions: HashMap::new(),
            markets: markets.into_iter().map(String::from).collect(),
            exchange,
        }
    }
}

#[async_trait]
impl<S: Strategy + 'static> Exchange<S> for Binance {
    async fn run(mut self, strategy: &mut S) {
        let (tx, rx) = mpsc::unbounded_channel();

        tokio::join!(self.produce_trades(tx), self.consume_trades(rx, strategy),);
    }
}

impl Binance {
    async fn connect_websocket(
        &self,
    ) -> OpenLimitsResult<
        BoxStream<
            'static,
            OpenLimitsResult<WebSocketResponse<<BinanceWebsocket as ExchangeWs>::Response>>,
        >,
    > {
        let subscriptions = self
            .markets
            .iter()
            .map(|symbol| Subscription::Trades(symbol.to_lowercase().to_string()))
            .collect::<Vec<Subscription>>();

        let stream = OpenLimitsWs {
            websocket: BinanceWebsocket::new(if self.sandbox {
                BinanceParameters::prod()
            } else {
                BinanceParameters::prod()
            })
            .await?,
        }
        .create_stream(&subscriptions)
        .await?;

        Ok(stream)
    }

    async fn produce_trades(&self, tx: UnboundedSender<Trade>) {
        loop {
            if let Ok(mut stream) = self.connect_websocket().await {
                log::info!("Trade stream started!");
                while let Ok(Some(Ok(message))) =
                    timeout(Duration::from_secs(5), stream.next()).await
                {
                    match message {
                        WebSocketResponse::Generic(OpenLimitsWebSocketMessage::Trades(trades)) => {
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
                log::warn!("Message timeout, trying to reconnect.");
            } else {
                log::warn!("Unable to reach websocket, trying to reconnect.");
                sleep(Duration::from_secs(5)).await;
            }
        }
    }

    async fn consume_trades(&self, mut rx: UnboundedReceiver<Trade>, strategy: &mut Strategy) {
        while let Some(trade) = rx.recv().await {
            if let Some(order) = strategy.run(trade) {
                if let Err(err) = self.order(order).await {
                    log::warn!("Error occured during order: {:#?}", err);
                } else {
                    log::info!("Order successful!");
                }
            }
        }
    }

    async fn order(&self, order: Order) -> OpenLimitsResult<()> {
        log::info!("Requesting order {}.", order);

        let price = Decimal::from_f32(order.price).unwrap();
        let take_profit = Decimal::from_f32(order.take_profit.unwrap()).unwrap();
        let stop_loss = Decimal::from_f32(order.stop_loss.unwrap()).unwrap();

        let balances = self.exchange.get_account_balances(None).await?;

        let balance = balances
            .iter()
            .filter(|balance| balance.asset == "USDT")
            .map(|balance| balance.free)
            .next()
            .unwrap_or(Decimal::zero());

        // TODO: Remove
        let balance = Decimal::from_f32(100.0).unwrap();

        if balance > Decimal::from_f32(30.0).unwrap() {
            log::info!("Placing entry order.");

            let buy_order = self
                .exchange
                .limit_buy(&OpenLimitOrderRequest {
                    market_pair: order.market.clone(),
                    size: balance / price,
                    price,
                    time_in_force: TimeInForce::FillOrKill,
                    post_only: false,
                })
                .await?;

            log::info!("Placing entry order was successful!");

            debug_assert_eq!(buy_order.status, openlimits::model::OrderStatus::Expired);

            let pair = self.exchange.get_pair(&order.market).await?.read()?;
            let incr = pair.quote_increment;

            let inner = self.exchange.inner_client().unwrap();

            log::info!("Placing OCO order.");

            inner
                .oco_sell(
                    pair,
                    buy_order.size,
                    take_profit,
                    stop_loss,
                    Some(stop_loss - incr),
                    Some(TimeInForce::GoodTillCancelled.into()),
                )
                .await?;

            log::info!("Placing OCO order was successful!");
        } else {
            log::info!("Balance not sufficient.")
        }

        Ok(())
    }
}
