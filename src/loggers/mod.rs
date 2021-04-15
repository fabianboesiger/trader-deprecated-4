mod telegram;

use crate::exchanges::binance::Position;
use async_trait::async_trait;
pub use telegram::Telegram;
use tokio::sync::mpsc::UnboundedSender;

pub type Sender = UnboundedSender<Message>;

pub enum Message {
    Open(Position),
    Close(Position, bool),
}

#[async_trait]
pub trait Logger: Sized {
    fn new() -> (Self, Sender);
    async fn run(&mut self);
}
