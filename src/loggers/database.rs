use super::{Logger, Message, Sender};
use async_trait::async_trait;
use std::env;
use tokio::sync::mpsc::{self, UnboundedReceiver};
use sqlx::PgPool;

pub struct Database {
    rx: UnboundedReceiver<Message>,
}

#[async_trait]
impl Logger for Database {
    fn new() -> (Database, Sender) {     
        let (sender, rx) = mpsc::unbounded_channel();

        (
            Database {
                rx,
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

impl Database {
    async fn run_internal(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Starting database logger.");

        let uri = env::var("DATABASE_URL").expect("Couldn't get DATABASE_URL.");
        let pool = PgPool::connect(&uri).await.unwrap();

        while let Some(message) = self.rx.recv().await {
            match message {
                Message::Open(position) => {
                    sqlx::query!(
                        r#"
                            INSERT INTO positions (timestamp, market, quantity, buy_price, take_profit, stop_loss, profitable) VALUES
                            ($1, $2, $3, $4, $5, $6, $7)
                        "#,
                        position.timestamp,
                        position.market,
                        position.quantity,
                        position.buy_price,
                        position.take_profit,
                        position.stop_loss,
                        position.profitable,
                    )
                    .execute(&pool)
                    .await?;
                },
                Message::Close(position) => {
                    sqlx::query!(
                        r#"
                            UPDATE positions SET profitable = $3 WHERE timestamp = $1 AND market = $2;
                        "#,
                        position.timestamp,
                        position.market,
                        position.profitable,
                    )
                    .execute(&pool)
                    .await?;
                }
            }
        }

        log::error!("Telegram logger terminated.");

        Ok(())
    }
}