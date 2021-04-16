use super::{Logger, Message, Sender};
use crate::exchanges::binance::Position;
use async_trait::async_trait;
use rust_decimal::prelude::*;
use std::env;
use telegram_bot::{requests::SendMessage, Api, ChannelId};
use tokio::sync::mpsc::{self, UnboundedReceiver};

pub struct Telegram {
    api: Api,
    rx: UnboundedReceiver<Message>,
    channel_id: ChannelId,
}

#[async_trait]
impl Logger for Telegram {
    fn new() -> (Telegram, Sender) {
        let token = env::var("TELEGRAM_BOT_TOKEN").expect("TELEGRAM_BOT_TOKEN not set.");
        let api = Api::new(token);

        let channel_id = env::var("TELEGRAM_CHANNEL_ID")
            .expect("TELEGRAM_CHANNEL_ID not set.")
            .parse::<i64>()
            .expect("Invalid Telegram user ID.");
        let channel_id = ChannelId::new(channel_id);

        let (sender, rx) = mpsc::unbounded_channel();

        (
            Telegram {
                api,
                rx,
                channel_id,
            },
            sender.into(),
        )
    }

    async fn run(&mut self) {
        log::info!("Starting logger.");

        /*
        self.api
            .send(SendMessage::new(
                self.channel_id,
                "Trading bot was started. Let's get this bread!",
            ))
            .await
            .unwrap();
        */

        while let Some(message) = self.rx.recv().await {
            match message {
                Message::Open(Position {
                    mut market,
                    quantity,
                    buy_price,
                    take_profit,
                    stop_loss,
                }) => {
                    log::info!("Logger received open position.");

                    let base_quantity = quantity / buy_price;
                    let base_take_profit = quantity / take_profit;
                    let base_stop_loss = quantity / stop_loss;
                    let usdt_offset = market.find("USDT").unwrap();
                    let asset: String = market.drain(..usdt_offset).collect();

                    self.api.send(SendMessage::new(
                        self.channel_id,
                        format!(
                                "🔵 Opened Position\n\nPrice:\t {:.2} {}\nValue:\t {:.4} {}\t ({:.2} USDT)\nTake Profit:\t {:.4} {}\t ({:.2} USDT)\nStop Loss:\t {:.4} {}\t ({:.2} USDT)",
                                buy_price, market,
                                base_quantity, asset, quantity,
                                base_take_profit, asset, take_profit,
                                base_stop_loss, asset, stop_loss
                            ),
                    )).await.unwrap();
                },
                Message::Close(
                    Position {
                        mut market,
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
                    let asset: String = market.drain(..usdt_offset).collect();

                    if profitable {
                        let profit = take_profit / buy_price - Decimal::one();
                        self.api.send(SendMessage::new(
                            self.channel_id,
                            format!(
                                    "🟢 Closed Position\n\nPrice:\t {:.2} {}\nValue:\t {:.4} {}\t ({:.2} USDT)\n\nProfit:\t {:+.2}%\t ({:+.2} USDT)",
                                    take_profit, market,
                                    base_quantity, asset, quantity,
                                    profit * Decimal::new(100, 0), profit * quantity
                                ),
                        )).await.unwrap();
                    } else {
                        let loss = stop_loss / buy_price - Decimal::one();
                        self.api.send(SendMessage::new(
                            self.channel_id,
                            format!(
                                    "🔴 Closed Position\n\nPrice:\t {:.2} {}\nValue:\t {:.4} {}\t ({:.2} USDT)\n\nLoss:\t {:+.2}%\t ({:+.2} USDT)",
                                    stop_loss, market,
                                    base_quantity, asset, quantity,
                                    loss * Decimal::new(100, 0), loss * quantity
                                ),
                        )).await.unwrap();
                    }
                }
            }
        }
    }
}
