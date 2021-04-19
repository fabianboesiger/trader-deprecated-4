use crate::{
    loggers::{Message, Sender},
    Market,
};
use chrono::{DateTime, Utc};
use rust_decimal::prelude::*;
use tokio::{
    sync::{
        Mutex,
    },
};

#[derive(Debug, Clone)]
pub struct Position {
    pub market: Market,
    pub quantity: Decimal,
    pub buy_price: Decimal,
    pub take_profit: Decimal,
    pub stop_loss: Decimal,
    pub profitable: Option<bool>,
    pub timestamp: DateTime<Utc>,
}

pub struct Positions {
    positions: Mutex<Vec<Position>>,
    sender: Sender,
}

impl Positions {
    pub fn new(sender: Sender) -> Self {
        Positions {
            positions: Mutex::new(Vec::new()),
            sender,
        }
    }

    pub async fn check(&self, market: &Market, price: Decimal) -> Option<bool> {
        let mut positions = self.positions.lock().await;

        let mut profitalbe = false;
        let mut index = None;
        for (i, position) in positions
            .iter()
            .enumerate()
            .filter(|(_, position)| position.market == *market)
        {
            if price <= position.stop_loss {
                index = Some(i);
            }
            if price >= position.take_profit {
                index = Some(i);
                profitalbe = true;
            }
        }

        if let Some(index) = index {
            let position = positions.get_mut(index).unwrap();
            position.profitable = Some(profitalbe);

            log::info!("Closing postion: {:?}", position);

            self.sender.send(Message::Close(positions.remove(index)));
            Some(profitalbe)
        } else {
            None
        }
    }

    pub async fn open(&self, position: Position) {
        log::info!("Opening postion: {:?}", position);
        self.sender.send(Message::Open(position.clone()));
        self.positions.lock().await.push(position);
    }
}