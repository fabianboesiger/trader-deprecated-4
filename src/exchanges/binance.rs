use super::{Exchange, Order, Strategy, Trade};
use crate::{Market, Number};
use async_trait::async_trait;
use futures::stream::{BoxStream, StreamExt};
use openlimits::{
    binance::{
        model::{SymbolFilter, websocket::{BinanceWebsocketMessage, UserOrderUpdate}}, Binance as OpenLimitsBinance, BinanceCredentials, BinanceParameters,
        BinanceWebsocket,
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
use tokio::sync::Mutex;
use std::{collections::HashMap};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::time::{sleep, timeout, Duration};

type OrderId = String;

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
struct Position {
    state: State,
    price: Decimal,
    quantity: Decimal,
    order: Option<FilteredOrder>,
}

impl Position {
    fn value(&self) -> Decimal {
        self.quantity * self.price
    }
}

#[derive(Default)]
struct Positions(Mutex<HashMap<Market, Position>>);

impl Positions {
    fn enter(&self, market: Market, order: FilteredOrder) {
        
    }

    async fn update_price(&self, market: Market, price: Decimal) {
        self.0.lock().await.entry(market).or_default().price = price;
    }

    async fn update_quantity(&self, market: Market, quantity: Decimal) {
        self.0.lock().await.entry(market).or_default().quantity = quantity;
    }


    async fn value(&self) -> Decimal {
        self.0.lock().await.values().map(Position::value).sum()
    }
    /*
    async fn update_positions(&self, exchange: &OpenLimitsBinance) -> Decimal {
        let guard = self.0.lock().await;

    }*/
}

struct FilteredOrder {
    buy_price: Decimal,
    take_profit_price: Decimal,
    stop_price: Decimal,
    stop_limit_price: Decimal,
    quantity: Decimal,
}

struct Filters(Vec<SymbolFilter>);

impl Filters {
    fn apply(&self, order: Order, quantity: Decimal) -> Result<FilteredOrder, ()> {
        let quantity = self.quantity(quantity)?;
        let stop_limit_price = self.price(order.stop_loss.unwrap(), quantity)?;

        Ok(FilteredOrder {
            buy_price: self.price(order.price, quantity)?,
            take_profit_price: self.price(order.take_profit.unwrap(), quantity)?,
            stop_price: stop_limit_price - self.tick_size()?,
            stop_limit_price,
            quantity,
        })
    }

    fn tick_size(&self) -> Result<Decimal, ()> {
        for filter in &self.0 {
            match filter {
                SymbolFilter::PriceFilter {
                    tick_size,
                    ..
                } => {
                    return Ok(*tick_size);
                },
                _ => ()
            }
        }

        Err(())
    }

    fn quantity(&self, mut quantity: Decimal) -> Result<Decimal, ()> {
        for filter in &self.0 {
            match filter {
                SymbolFilter::LotSize {
                    min_qty,
                    max_qty,
                    step_size,
                } => {
                    if quantity < *min_qty {
                        return Err(());
                    }
                    if quantity > *max_qty {
                        return Err(());
                    }
                    quantity = (quantity / step_size).round_dp_with_strategy(
                        0,
                        RoundingStrategy::RoundDown
                    ) * step_size;
                },
                SymbolFilter::MarketLotSize {
                    min_qty,
                    max_qty,
                    step_size,
                } => {
                    if quantity < *min_qty {
                        return Err(());
                    }
                    if quantity > *max_qty {
                        return Err(());
                    }
                    quantity = (quantity / step_size).round_dp_with_strategy(
                        0,
                        RoundingStrategy::RoundDown
                    ) * step_size;
                },
                _ => ()
            }
        }

        Ok(quantity)
    }

    fn price(&self, price: Number, quantity: Decimal) -> Result<Decimal, ()> {
        let mut price = Decimal::from_f32(price).unwrap();

        for filter in &self.0 {
            match filter {
                SymbolFilter::PriceFilter {
                    min_price,
                    max_price,
                    tick_size,
                } => {
                    if price < *min_price {
                        return Err(());
                    }
                    if price > *max_price {
                        return Err(());
                    }
                    price = (price / tick_size).round() * tick_size;
                },
                SymbolFilter::MinNotional {
                    min_notional
                } => {
                    if price * quantity < *min_notional {
                        return Err(());
                    }
                },
                _ => ()
            }
        }

        Ok(price)
    }
}

pub struct Binance {
    sandbox: bool,
    positions: Positions,
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
                 }| {
                    (symbol, Filters(filters))
                 },
            )
            .collect();

        println!(
            "{:?}",
            exchange
                .retrieve_pairs()
                .await
                .unwrap()
                .into_iter()
                .map(|pair| pair.symbol)
                .collect::<Vec<String>>()
        );

        Self {
            sandbox,
            positions: Default::default(),
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
                while let Ok(Some(Ok(message))) =
                    timeout(Duration::from_secs(if self.sandbox { 500 } else { 5 }), stream.next()).await
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
                        WebSocketResponse::Raw(BinanceWebsocketMessage::UserOrderUpdate(UserOrderUpdate {
                            order_status,
                            ..
                        })) => {

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
            let timestamp = trade.timestamp;

            self.positions.update_price(
                trade.market.clone(),
                Decimal::from_f32(trade.price).unwrap(),
            ).await;

            if let Some(order) = strategy.run(trade) {
                if timestamp as u64 >= self.start + 1000 * 60 * 60 * 8 {
                    if let Err(err) = self.order(order).await {
                        log::warn!("Error occured during order: {:#?}", err);
                    }
                }
            }
        }
    }

    async fn order(&self, order: Order) -> OpenLimitsResult<()> {
        log::info!("Requesting order {}.", order);

        //let filters = self.filters.get(&order.market).unwrap();
        //filters.apply(order, quantity).unwrap();

        let price = Decimal::from_f32(order.price).unwrap();
        let take_profit = Decimal::from_f32(order.take_profit.unwrap()).unwrap();
        let stop_loss = Decimal::from_f32(order.stop_loss.unwrap()).unwrap();

        let pair = self.exchange.get_pair(&order.market).await?.read()?;
        let balances = self.exchange.get_account_balances(None).await?;
        let incr = pair.quote_increment.normalize();

        for balance in &balances {
            self.positions.update_quantity(format!("{}USDT", balance.asset), balance.total).await;
        }

        let quote_balance = balances
            .iter()
            .filter(|balance| balance.asset == pair.quote)
            .map(|balance| balance.free)
            .next()
            .unwrap_or(Decimal::zero());

        let base_balance = balances
            .iter()
            .filter(|balance| balance.asset == pair.base)
            .map(|balance| balance.free)
            .next()
            .unwrap_or(Decimal::zero());

        if base_balance * price < Decimal::new(10, 0) {
            let total = self.positions.value().await;
            let min_investment = total / Decimal::new(3, 0);
            let investment = determine_investment_amount(min_investment, quote_balance);

            if investment > Decimal::zero() {
                log::info!("Placing entry order.");
    
                let buy_order = self
                    .exchange
                    .limit_buy(&OpenLimitOrderRequest {
                        market_pair: order.market.clone(),
                        size: investment / price * Decimal::new(999, 3),
                        price: price + incr,
                        time_in_force: TimeInForce::FillOrKill,
                        post_only: false,
                    })
                    .await?;

                log::info!("Placing entry order was successful!");

                if buy_order.status == openlimits::model::OrderStatus::Filled {
                    log::info!("Entry order was filled.");

                    
                    let inner = self.exchange.inner_client().unwrap();

                    log::info!("Placing OCO order.");

                    inner
                        .oco_sell(
                            pair,
                            buy_order.size,
                            take_profit,
                            stop_loss - incr,
                            Some(stop_loss),
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
        } else {
            log::info!("Already invested in this asset.");
        }
        
        Ok(())
    }
}

fn determine_investment_amount(minimum: Decimal, available: Decimal) -> Decimal {
    let fraction_investment = available / minimum;
    if fraction_investment >= Decimal::new(2, 0) {
        minimum
    } else
    if fraction_investment >= Decimal::new(1, 0) {
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
    fn test_investment_amount() {
        assert_eq!(determine_investment_amount(Decimal::new(1, 0), Decimal::new(2, 0)), Decimal::new(1, 0));
        assert_eq!(determine_investment_amount(Decimal::new(10, 0), Decimal::new(1, 0)), Decimal::new(0, 0));
        assert_eq!(determine_investment_amount(Decimal::new(1, 0), Decimal::new(1, 0)), Decimal::new(1, 0));
        assert_eq!(determine_investment_amount(Decimal::new(15, 0), Decimal::new(20, 0)), Decimal::new(20, 0));
    }
}
