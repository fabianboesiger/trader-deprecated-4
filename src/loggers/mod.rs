mod telegram;
mod database;

use crate::exchanges::binance::Position;
use async_trait::async_trait;
pub use telegram::Telegram;
pub use database::Database;
use tokio::sync::mpsc::UnboundedSender;

pub struct Sender(UnboundedSender<Message>);

impl Sender {
    pub fn send(&self, message: Message) {
        if let Err(err) = self.0.send(message) {
            log::error!("Logger error: {}", err);
        }
    }
}

impl From<UnboundedSender<Message>> for Sender {
    fn from(sender: UnboundedSender<Message>) -> Self {
        Sender(sender)
    }
}

pub enum Message {
    Open(Position),
    Close(Position),
}

#[async_trait]
pub trait Logger: Sized {
    fn new() -> (Self, Sender);
    async fn run(mut self);
}
