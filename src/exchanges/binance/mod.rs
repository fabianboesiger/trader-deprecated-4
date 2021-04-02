use super::{Exchange, Order, Strategy, Trade};
use crate::{Error, Market, Number};
use async_trait::async_trait;
use futures::{stream::BoxStream, StreamExt};
use openlimits::{
    binance::{
        model::{
            websocket::{BinanceWebsocketMessage, UserOrderUpdate},
            SymbolFilter,
        },
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
use tokio::sync::Mutex;
use tokio::time::{sleep, timeout, Duration};

type OrderId = String;
type Symbol = String;

enum State {
    Wait,
    Buy(OrderId),
    Hold,
    Sell,
}

impl Default for State {
    fn default() -> State {
        State::Wait
    }
}

#[derive(Default)]
struct Asset {
    price: Decimal,
    quantity: Decimal,
}

impl Asset {
    fn value(&self) -> Decimal {
        self.quantity * self.price
    }
}

struct Wallet(Mutex<HashMap<Symbol, Asset>>);

impl Wallet {
    pub const QUOTE: &'static str = "USDT";

    fn new() -> Self {
        let mut map = HashMap::new();
        map.insert(
            Wallet::QUOTE.to_owned(),
            Asset {
                price: Decimal::one(),
                quantity: Decimal::zero(),
            },
        );

        Wallet(Mutex::new(map))
    }

    async fn update(&self, exchange: &OpenLimitsBinance) -> Result<(), Error> {
        let balances = exchange.get_account_balances(None).await?;

        let mut wallet = self.0.lock().await;
        for balance in balances {
            wallet.entry(balance.asset).or_default().quantity = balance.total;
        }

        Ok(())
    }

    async fn update_price(&self, mut market: Market, price: Decimal) {
        let usdt_offset = market.find(Wallet::QUOTE).unwrap();
        self.0
            .lock()
            .await
            .entry(market.drain(..usdt_offset).collect())
            .or_default()
            .price = price;
    }

    async fn update_quantity(&self, asset: Symbol, quantity: Decimal) {
        self.0.lock().await.entry(asset).or_default().quantity = quantity;
    }

    async fn value<A: AsRef<str>>(&self, asset: A) -> Decimal {
        self.0
            .lock()
            .await
            .get(asset.as_ref())
            .map(|position| position.value())
            .unwrap_or(Decimal::zero())
    }

    async fn total_value(&self) -> Decimal {
        self.0.lock().await.values().map(Asset::value).sum()
    }
    /*
    async fn update_positions(&self, exchange: &OpenLimitsBinance) -> Decimal {
        let guard = self.0.lock().await;

    }*/
}

struct FilteredOrder {
    market: Market,
    buy_price: Decimal,
    take_profit_price: Decimal,
    stop_price: Decimal,
    stop_limit_price: Decimal,
    quote_quantity: Decimal,
}

impl FilteredOrder {
    async fn order(self, exchange: &OpenLimitsBinance) -> OpenLimitsResult<()> {
        if self.quote_quantity > Decimal::zero() {
            log::info!("Placing entry order.");

            let base_quantity = self.quote_quantity / self.buy_price;

            let buy_order = exchange
                .limit_buy(&OpenLimitOrderRequest {
                    market_pair: self.market.clone(),
                    size: base_quantity * Decimal::new(999, 3),
                    price: self.buy_price,
                    time_in_force: TimeInForce::ImmediateOrCancelled,
                    post_only: false,
                })
                .await?;

            log::info!("Placing entry order was successful!");

            if buy_order.status == openlimits::model::OrderStatus::Filled {
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

                log::info!("Placing OCO order was successful!");
            } else {
                log::info!("Entry order was killed.");
            }
        } else {
            log::info!("Balance not sufficient.")
        }

        Ok(())
    }
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

        Err(Error::Filter)
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
                        return Err(Error::Filter);
                    }
                    if quantity > max_qty {
                        return Err(Error::Filter);
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
                        return Err(Error::Filter);
                    }
                    if quantity > max_qty {
                        return Err(Error::Filter);
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
                        return Err(Error::Filter);
                    }
                    if price > max_price {
                        return Err(Error::Filter);
                    }
                    price = (price / tick_size).round() * tick_size;
                }
                SymbolFilter::MinNotional { min_notional } => {
                    if price * quantity < *min_notional {
                        return Err(Error::Filter);
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
    markets: Vec<Market>,
    exchange: OpenLimitsBinance,
    filters: HashMap<Market, Filters>,
    start: u64,
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
        let start = inner.get_server_time().await.unwrap().server_time;
        let info = inner.get_exchange_info().await.unwrap();
        let filters = info
            .symbols
            .into_iter()
            .map(
                |openlimits::binance::model::Symbol {
                     symbol, filters, ..
                 }| { (symbol, Filters(filters)) },
            )
            .collect();

        Self {
            sandbox,
            wallet: Wallet::new(),
            markets: markets.into_iter().map(String::from).collect(),
            exchange,
            filters,
            start,
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

                                let trade = Trade {
                                    market,
                                    quantity: quantity.to_f32().unwrap(),
                                    price: price.to_f32().unwrap(),
                                    timestamp,
                                };

                                tx.send(trade).unwrap();
                            }
                        }
                        WebSocketResponse::Raw(BinanceWebsocketMessage::UserOrderUpdate(
                            UserOrderUpdate { .. },
                        )) => {}
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
                if let Err(err) = self.order(order).await {
                    log::warn!("Error occured during order: {:#?}", err);
                }
                //} else {
                //    log::warn!("Too early to order something!");
                //}
            }
        }
    }

    async fn order(&self, order: Order) -> Result<(), Error> {
        log::info!("Requesting order {}.", order);

        //let filters = self.filters.get(&order.market).unwrap();
        //filters.apply(order, quantity).unwrap();

        self.wallet.update(&self.exchange).await?;
        let pair = self.exchange.get_pair(&order.market).await?.read()?;

        if self.wallet.value(pair.base).await < Decimal::new(10, 0) {
            let total = self.wallet.total_value().await;
            let min_quantity =
                (total - Decimal::new(50, 0)) / Decimal::new(2, 0) * Decimal::new(99, 2);
            let quantity =
                determine_investment_amount(min_quantity, self.wallet.value(Wallet::QUOTE).await);
            let filtered_order = self
                .filters
                .get(&order.market)
                .unwrap()
                .apply(order, quantity)?;
            filtered_order.order(&self.exchange).await?;
        } else {
            log::info!("Already invested in this asset.");
        }

        Ok(())
    }
}

fn determine_investment_amount(minimum: Decimal, available: Decimal) -> Decimal {
    assert!(minimum > Decimal::zero());

    let fraction_investment = available / minimum;
    if fraction_investment >= Decimal::new(2, 0) {
        minimum
    } else if fraction_investment >= Decimal::new(1, 0) {
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
    }

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
            .update_quantity(Wallet::QUOTE.to_owned(), Decimal::new(10000, 0))
            .await;
        assert_eq!(wallet.total_value().await, Decimal::new(60000, 0));
    }
}
