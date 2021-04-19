use super::{Logger, Message, Sender};
use crate::exchanges::binance::Position;
use async_trait::async_trait;
use rust_decimal::prelude::*;
use std::env;
use telegram_bot::{requests::SendMessage, Api, ChannelId};
use tokio::sync::mpsc::{self, UnboundedReceiver};

pub struct Telegram {
    pool: PgPool,
    rx: UnboundedReceiver<Message>,
    channel_id: ChannelId,
}

#[async_trait]
impl Logger for Telegram {
    fn new() -> (Telegram, Sender) {
        let uri = std::env::var("DATABASE_URL").expect("Couldn't get DATABASE_URL.");
        let pool = PgPool::connect(&uri).await.unwrap();

        let (sender, rx) = mpsc::unbounded_channel();

        (
            Telegram {
                pool,
                rx,
                channel_id,
            },
            sender.into(),
        )
    }

    async fn run(mut self) {
        loop {
            if let Err(err) = self.run_internal().await {
                log::error!("Database error: {}", err);
            }
        }
    }


}

impl Telegram {
    async fn run_internal(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Starting logger.");

        let api = Api::new(self.token.clone());
        /*
        api
            .send(SendMessage::new(
                self.channel_id,
                "Trading bot was started. Let's get this bread!",
            ))
            .await?;*/

        while let Some(message) = self.rx.recv().await {
            match message {
                Message::Open(Position {
                    market,
                    quantity,
                    buy_price,
                    take_profit,
                    stop_loss,
                }) => {
                    log::info!("Logger received open position");

                    let base_quantity = quantity / buy_price;
                    let usdt_offset = market.find("USDT").unwrap();
                    let asset: String = market.clone().drain(..usdt_offset).collect();

                    api.send(SendMessage::new(
                        self.channel_id,
                        format!(
                                "🔵 Opened Position {}\n\nQuantity: {:.4} {} ({:.2} USDT)\nBuy Price: {:.4} USDT\nTake Profit: {:.4} USDT\nStop Loss: {:.4} USDT",
                                market, base_quantity, asset, quantity, buy_price, take_profit, stop_loss
                            ),
                    )).await?;
                },
                Message::Close(
                    Position {
                        market,
                        quantity,
                        buy_price,
                        take_profit,
                        stop_loss,
                    },
                    profitable,
                ) => {
                    log::info!("Logger received close position.");

                    let base_quantity = quantity / buy_price;
                    let usdt_offset = market.find("USDT").unwrap();
                    let asset: String = market.clone().drain(..usdt_offset).collect();

                    if profitable {
                        let profit = take_profit / buy_price - Decimal::one();
                        api.send(SendMessage::new(
                            self.channel_id,
                            format!(
                                    "🟢 Closed Position {}\n\nQuantity:\t {:.4} {}\t ({:.2} USDT)\nSell Price:\t {:.4} USDT\nProfit:\t {:+.2}%\t ({:+.2} USDT)",
                                    market,
                                    base_quantity, asset, quantity,
                                    take_profit,
                                    profit * Decimal::new(100, 0), profit * quantity
                                ),
                        )).await?;
                    } else {
                        let loss = stop_loss / buy_price - Decimal::one();
                        api.send(SendMessage::new(
                            self.channel_id,
                            format!(
                                    "🔴 Closed Position {}\n\nQuantity:\t {:.4} {}\t ({:.2} USDT)\nSell Price:\t {:.4} USDT\nLoss:\t {:+.2}%\t ({:+.2} USDT)",
                                    market,
                                    base_quantity, asset, quantity,
                                    stop_loss,
                                    loss * Decimal::new(100, 0), loss * quantity
                                ),
                        )).await?;
                    }
                }
            }
        }

        log::error!("Telegram logger terminated.");

        Ok(())
    }
}