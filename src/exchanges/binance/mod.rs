mod wallet;
mod positions;

pub use positions::Position;
use positions::Positions;
use super::{Exchange, Order, Strategy, Trade};
use crate::{
    loggers::{Logger, Database},
    Error, Market, Number,
};
use async_trait::async_trait;
use futures::{stream::BoxStream, StreamExt};
use openlimits::{
    binance::{
        model::{websocket::BinanceSubscription, SymbolFilter},
        Binance as OpenLimitsBinance, BinanceCredentials, BinanceParameters, BinanceWebsocket,
    },
    exchange::{Exchange as OpenLimitsExchange, ExchangeAccount},
    exchange_info::ExchangeInfoRetrieval,
    exchange_ws::{ExchangeWs, OpenLimitsWs},
    model::{
        websocket::{OpenLimitsWebSocketMessage, WebSocketResponse},
        Side,
    },
    model::{OpenMarketOrderRequest, OrderStatus, TimeInForce},
    shared::Result as OpenLimitsResult,
};
use rust_decimal::prelude::*;
use std::{
    collections::HashMap,
    sync::atomic::{AtomicU64, AtomicU8, Ordering},
};
use tokio::{
    sync::{
        mpsc::{self, UnboundedReceiver, UnboundedSender},
    },
    time::{sleep, timeout, Duration},
};
use wallet::Wallet;
use chrono::Utc;

#[derive(Debug)]
struct FilteredOrder {
    market: Market,
    buy_price: Decimal,
    take_profit_price: Decimal,
    stop_price: Decimal,
    stop_limit_price: Decimal,
    quote_quantity: Decimal,
}

impl FilteredOrder {
    #[cfg(feature = "stop-orders")]
    async fn order(self, exchange: &OpenLimitsBinance) -> OpenLimitsResult<Option<Position>> {
        Ok(Some(Position {
            market: self.market,
            quantity: self.quote_quantity,
            buy_price: self.buy_price,
            take_profit: self.take_profit_price,
            stop_loss: self.stop_price,
            profitable: None,
            timestamp: Utc::now(),
        }))
    }

    #[cfg(not(feature = "stop-orders"))]
    async fn order(self, exchange: &OpenLimitsBinance) -> OpenLimitsResult<Option<Position>> {
        log::info!("FilteredOrder: {:#?}", self);

        Ok(if self.quote_quantity > Decimal::zero() {
            log::info!("Placing entry order.");

            let base_quantity = self.quote_quantity / self.buy_price;

            let buy_order = exchange
                .market_buy(&OpenMarketOrderRequest {
                    market_pair: self.market.clone(),
                    size: base_quantity * Decimal::new(999, 3),
                    //price: self.buy_price,
                    //time_in_force: TimeInForce::ImmediateOrCancelled,
                    //post_only: false,
                })
                .await?;

            log::info!("Placing entry order was successful!");

            if buy_order.status == OrderStatus::Filled {
                log::info!("Entry order was filled.");

                let inner = exchange.inner_client().unwrap();
                let pair = exchange.get_pair(&self.market).await?.read()?;

                log::info!("Placing OCO order.");

                inner
                    .oco_sell(
                        pair,
                        buy_order.size,
                        self.take_profit_price,
                        self.stop_price,
                        Some(self.stop_limit_price),
                        Some(TimeInForce::GoodTillCancelled.into()),
                    )
                    .await?;

                //let take_profit_id = result.order_reports.iter().filter(|order| order.type_name == "LIMIT_MAKER").map(|order| order.order_id).next().unwrap();
                //let stop_loss_id = result.order_reports.iter().filter(|order| order.type_name == "STOP_LOSS_LIMIT").map(|order| order.order_id).next().unwrap();

                log::info!("Placing OCO order was successful!");

                Some(Position {
                    market: self.market,
                    quantity: buy_order.size,
                    buy_price: self.buy_price,
                    take_profit: self.take_profit_price,
                    stop_loss: self.stop_limit_price,
                    profitable: None,
                    timestamp: Utc::now()
                })
            } else {
                log::info!("Entry order was killed.");
                None
            }
        } else {
            log::info!("Balance not sufficient.");
            None
        })
    }
}

#[derive(Debug)]
pub enum FilterError {
    MinQty,
    MaxQty,
    MinPrice,
    MaxPrice,
    MinNotional,
    NoTickSize,
}

struct Filters(Vec<SymbolFilter>);

impl Filters {
    fn apply(&self, order: Order, quantity: Decimal) -> Result<FilteredOrder, Error> {
        let quantity = self.quantity(quantity)?;
        let stop_limit_price = self.price(order.stop_loss.unwrap(), quantity)?;
        let tick = self.tick_size()?;

        Ok(FilteredOrder {
            market: order.market,
            buy_price: self.price(order.price, quantity)? + Decimal::new(2, 0) * tick,
            take_profit_price: self.price(order.take_profit.unwrap(), quantity)?,
            stop_price: stop_limit_price - Decimal::new(2, 0) * tick,
            stop_limit_price,
            quote_quantity: quantity,
        })
    }

    fn tick_size(&self) -> Result<Decimal, Error> {
        for filter in &self.0 {
            match filter {
                SymbolFilter::PriceFilter { tick_size, .. } => {
                    return Ok(*tick_size);
                }
                _ => (),
            }
        }

        Err(Error::Filter(FilterError::NoTickSize))
    }

    fn quantity(&self, mut quantity: Decimal) -> Result<Decimal, Error> {
        for filter in &self.0 {
            match filter {
                &SymbolFilter::LotSize {
                    min_qty,
                    max_qty,
                    step_size,
                } if step_size > Decimal::zero() => {
                    if quantity < min_qty {
                        return Err(Error::Filter(FilterError::MinQty));
                    }
                    if quantity > max_qty {
                        return Err(Error::Filter(FilterError::MaxQty));
                    }
                    quantity = (quantity / step_size)
                        .round_dp_with_strategy(0, RoundingStrategy::RoundDown)
                        * step_size;
                }
                &SymbolFilter::MarketLotSize {
                    min_qty,
                    max_qty,
                    step_size,
                } if step_size > Decimal::zero() => {
                    if quantity < min_qty {
                        return Err(Error::Filter(FilterError::MinQty));
                    }
                    if quantity > max_qty {
                        return Err(Error::Filter(FilterError::MaxQty));
                    }
                    quantity = (quantity / step_size)
                        .round_dp_with_strategy(0, RoundingStrategy::RoundDown)
                        * step_size;
                }
                _ => (),
            }
        }

        Ok(quantity)
    }

    fn price(&self, price: Number, quantity: Decimal) -> Result<Decimal, Error> {
        let mut price = Decimal::from_f32(price).unwrap();

        for filter in &self.0 {
            match filter {
                &SymbolFilter::PriceFilter {
                    min_price,
                    max_price,
                    tick_size,
                } if tick_size > Decimal::zero() => {
                    if price < min_price {
                        return Err(Error::Filter(FilterError::MinPrice));
                    }
                    if price > max_price {
                        return Err(Error::Filter(FilterError::MaxPrice));
                    }
                    price = (price / tick_size).round() * tick_size;
                }
                SymbolFilter::MinNotional { min_notional } => {
                    if price * quantity < *min_notional {
                        return Err(Error::Filter(FilterError::MinNotional));
                    }
                }
                _ => (),
            }
        }

        Ok(price)
    }
}

pub struct Binance {
    sandbox: bool,
    wallet: Wallet,
    positions: Positions,
    markets: Vec<Market>,
    exchange: OpenLimitsBinance,
    //filters: HashMap<Market, Filters>,
    consecutive_losses: AtomicU8,
    wait_until: AtomicU64,
    //start: u64,
}

impl Binance {
    async fn get_filters(&self) -> HashMap<Market, Filters> {
        let inner = self.exchange.inner_client().unwrap();
        let info = inner.get_exchange_info().await.unwrap();
        info
            .symbols
            .into_iter()
            .map(
                |openlimits::binance::model::Symbol {
                     symbol, filters, ..
                 }| { (symbol, Filters(filters)) },
            )
            .collect()
    }

    pub async fn new(markets: &Vec<&str>, sandbox: bool) -> Self {
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
        let start = inner.get_server_time().await.unwrap().server_time;
        //let info = inner.get_exchange_info().await.unwrap();
        /*let filters = info
            .symbols
            .into_iter()
            .map(
                |openlimits::binance::model::Symbol {
                     symbol, filters, ..
                 }| { (symbol, Filters(filters)) },
            )
            .collect();*/

        let (logger, sender) = Database::new();
        tokio::task::spawn(async move {
            logger.run().await;
        });

        Self {
            sandbox,
            wallet: Wallet::new(),
            positions: Positions::new(sender),
            markets: markets.clone().into_iter().map(String::from).collect(),
            exchange,
            //filters,
            consecutive_losses: AtomicU8::new(0),
            wait_until: AtomicU64::new(start),
            //start,
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
            .map(|symbol| BinanceSubscription::Trade(symbol.to_lowercase().to_string()))
            .collect::<Vec<BinanceSubscription>>();

        //let inner = self.exchange.inner_client().unwrap();
        //let user_data = inner.user_stream_start().await?;

        //subscriptions.push(BinanceSubscription::UserData(user_data.listen_key.clone()));

        let stream = OpenLimitsWs {
            websocket: BinanceWebsocket::new(if self.sandbox {
                BinanceParameters::sandbox()
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
                /*
                tokio::select! {
                    _ = async {
                        let mut interval = tokio::time::interval(Duration::from_secs(60 * 30));
                        loop {
                            interval.tick().await;
                            if let Err(err) = self.exchange.inner_client().unwrap().user_stream_keep_alive(&listen_key).await {
                                println!("{}\n {:#?}", listen_key, err);
                                break;
                            }
                        }
                        log::warn!("User data stream timeout, trying to reconnect.");
                    } => {}
                    _ = async {
                        */
                log::info!("Trade stream started!");
                while let Ok(Some(Ok(message))) = timeout(
                    Duration::from_secs(if self.sandbox { 500 } else { 5 }),
                    stream.next(),
                )
                .await
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

                                if let Some(profitable) = self.positions.check(&market, price).await
                                {
                                    if profitable {
                                        log::info!("Last trade was profitable.");
                                        self.consecutive_losses.store(0, Ordering::Relaxed);
                                    } else {
                                        log::info!("Last trade was unprofitable.");
                                        self.consecutive_losses.fetch_add(1, Ordering::Relaxed);
                                        if self.consecutive_losses.load(Ordering::Relaxed) >= 2 {
                                            self.wait_until.store(
                                                timestamp as u64 + 1000 * 60 * 60 * 24,
                                                Ordering::Relaxed,
                                            );
                                            self.consecutive_losses.store(0, Ordering::Relaxed);
                                        }
                                    }
                                }

                                let trade = Trade {
                                    market,
                                    quantity: quantity.to_f32().unwrap(),
                                    price: price.to_f32().unwrap(),
                                    timestamp,
                                };

                                tx.send(trade).unwrap();
                            }
                        }
                        /*
                        WebSocketResponse::Raw(BinanceWebsocketMessage::UserOrderUpdate(
                            UserOrderUpdate { trade_id, order_status, event_time, .. },
                        )) => {
                            if order_status == openlimits::binance::model::OrderStatus::Filled || order_status == openlimits::binance::model::OrderStatus::PartiallyFilled {
                                if let Some(profitable) = self.positions.terminate(trade_id as u64).await {
                                    if profitable {
                                        log::info!("Last trade was profitable.");
                                        self.consecutive_losses.store(0, Ordering::Relaxed);
                                    } else {
                                        log::info!("Last trade was unprofitable.");
                                        self.consecutive_losses.fetch_add(1, Ordering::Relaxed);
                                        if self.consecutive_losses.load(Ordering::Relaxed) >= 2 {
                                            self.wait_until.store(event_time + 1000 * 60 * 60 * 12, Ordering::Relaxed);
                                            self.consecutive_losses.store(0, Ordering::Relaxed);
                                        }
                                    }
                                }
                            }
                        }
                        */
                        _ => (),
                    }
                }
                log::warn!("Message timeout, trying to reconnect.");
                //} => {}
                //}
            } else {
                log::warn!("Unable to reach websocket, trying to reconnect.");
                sleep(Duration::from_secs(5)).await;
            }
        }
    }

    async fn consume_trades<S: Strategy + 'static>(
        &self,
        mut rx: UnboundedReceiver<Trade>,
        strategy: &mut S,
    ) {
        while let Some(trade) = rx.recv().await {
            log::trace!("Receiving trade: {:?}", trade);

            let timestamp = trade.timestamp;

            self.wallet
                .update_price(
                    trade.market.clone(),
                    Decimal::from_f32(trade.price).unwrap(),
                )
                .await;

            if let Some(order) = strategy.run(trade) {
                log::info!("Processing order.");
                //if timestamp as u64 >= self.start + 1000 * 60 * 60 * 4 {
                if let Err(err) = self.order(order, timestamp as u64).await {
                    log::error!("Error occured during order: {:#?}", err);
                }
                //} else {
                //    log::warn!("Too early to order something!");
                //}
            }
        }
    }

    async fn order(&self, order: Order, timestamp: u64) -> Result<(), Error> {
        log::info!("Requesting order {}.", order);

        //let filters = self.filters.get(&order.market).unwrap();
        //filters.apply(order, quantity).unwrap();

        self.get_filters().await;
        self.wallet.update(&self.exchange).await?;
        log::trace!("Wallet: {:#?}", self.wallet);
        let pair = self.exchange.get_pair(&order.market).await?.read()?;

        let base_quantity = self.wallet.value(pair.base.clone()).await;
        let not_invested = (pair.base == Wallet::FEE_ASSET && base_quantity < Decimal::new(60, 0))
            || base_quantity < Decimal::new(10, 0);
        if not_invested {
            if self.wait_until.load(Ordering::Relaxed) < timestamp {
                let total = self.wallet.total_value().await;
                let want =
                    (total - Decimal::new(50, 0)) / Decimal::new(2, 0);
                let available = self.wallet.value(Wallet::QUOTE_ASSET).await;
                    log::info!("Total value is {}, want {}, available {}", total, want, available);
                let quantity = determine_investment_amount(
                    want,
                    available,
                ) * Decimal::new(99, 2);
                log::info!("Placing order of size {}", quantity);
                let filtered_order = self.get_filters()
                    .await
                    .get(&order.market)
                    .unwrap()
                    .apply(order, quantity)?;

                if let Some(position) = filtered_order.order(&self.exchange).await? {
                    self.positions.open(position).await;
                }
            } else {
                log::warn!("Backoff still in place.")
            }
        } else {
            log::info!("Already invested in this asset.");
        }

        Ok(())
    }
}

fn determine_investment_amount(want: Decimal, available: Decimal) -> Decimal {
    assert!(want > Decimal::zero());

    let fraction_investment = available / want;
    if fraction_investment >= Decimal::new(2, 0) {
        want
    } else if fraction_investment >= Decimal::new(1, 0) {
        available
    } else if fraction_investment >= Decimal::new(5, 1) {
        available
    } else {
        Decimal::zero()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    #[test]
    fn investment_amount() {
        assert_eq!(
            determine_investment_amount(Decimal::new(1, 0), Decimal::new(2, 0)),
            Decimal::new(1, 0)
        );
        assert_eq!(
            determine_investment_amount(Decimal::new(10, 0), Decimal::new(1, 0)),
            Decimal::new(0, 0)
        );
        assert_eq!(
            determine_investment_amount(Decimal::new(1, 0), Decimal::new(1, 0)),
            Decimal::new(1, 0)
        );
        assert_eq!(
            determine_investment_amount(Decimal::new(15, 0), Decimal::new(20, 0)),
            Decimal::new(20, 0)
        );
        assert_eq!(
            determine_investment_amount(Decimal::new(75, 0), Decimal::new(160, 0)),
            Decimal::new(75, 0)
        );
        assert_eq!(
            determine_investment_amount(Decimal::new(70, 0), Decimal::new(50, 0)),
            Decimal::new(50, 0)
        );
        assert_eq!(
            determine_investment_amount(Decimal::new(80, 0), Decimal::new(230, 0)),
            Decimal::new(80, 0)
        );
        assert_eq!(
            determine_investment_amount(Decimal::new(80, 0), Decimal::new(160, 0)),
            Decimal::new(80, 0)
        );
        assert_eq!(
            determine_investment_amount(Decimal::new(80, 0), Decimal::new(40, 0)),
            Decimal::new(40, 0)
        );
    }
}
