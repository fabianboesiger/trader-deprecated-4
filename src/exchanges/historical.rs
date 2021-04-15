use super::{Exchange, Strategy, Trade};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::postgres::PgPool;
use std::time::SystemTime;
use tokio::{
    fs::OpenOptions,
    io::{AsyncReadExt, AsyncWriteExt},
};

pub struct Historical {
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    cache: bool,
}

impl Historical {
    pub fn new(from: DateTime<Utc>, to: DateTime<Utc>, cache: bool) -> Self {
        Self { from, to, cache }
    }
}

#[async_trait]
impl<S: Strategy + 'static> Exchange<S> for Historical {
    async fn run(self, strategy: &mut S) {
        let uri = std::env::var("DATABASE_URL").expect("Couldn't get DATABASE_URL.");
        let pool = PgPool::connect(&uri).await.unwrap();

        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open("trades.bin")
            .await
            .unwrap();

        let metadata = file.metadata().await.unwrap();

        let trades = if metadata.len() > 0
            && SystemTime::now()
                .duration_since(metadata.modified().unwrap())
                .unwrap()
                .as_secs()
                < 3600 * 4
            && self.cache
        {
            let mut bin = Vec::new();
            file.read_to_end(&mut bin).await.unwrap();
            bincode::deserialize(&bin[..]).unwrap()
        } else {
            let trades = sqlx::query_as!(
                Trade,
                r#"
                WITH
                grouped AS (SELECT
                    market,
                    CAST(SUM(quantity) AS REAL) AS quantity,
                    CAST(AVG(price) AS REAL) AS price,
                    timestamp/1000/60 AS timestamp
                FROM trades
                WHERE market = ANY($1)
                AND timestamp > $2
                AND timestamp <= $3
                GROUP BY market, timestamp/1000/60)
                SELECT
                    market AS "market!",
                    quantity AS "quantity!",
                    price AS  "price!",
                    timestamp*1000*60 AS "timestamp!"
                FROM grouped
                ORDER BY timestamp ASC"#,
                &vec![
                    "BTCUSDT",
                    "ETHUSDT",
                    "CHZUSDT",
                    "BNBUSDT",
                    "DOGEUSDT",
                    "ADAUSDT",
                    "BCHUSDT",
                    "XRPUSDT",
                    "LTCUSDT",
                    "EOSUSDT",
                    "DOTUSDT",
                    "THETAUSDT",
                    "LINKUSDT"
                ]
                .into_iter()
                .map(String::from)
                .collect::<Vec<String>>(),
                self.from.timestamp_millis(),
                self.to.timestamp_millis(),
            )
            .fetch_all(&pool)
            .await
            .unwrap();

            if self.cache {
                let bin = bincode::serialize(&trades).unwrap();
                file.write_all(&bin[..]).await.unwrap();
            }

            trades
        };

        for trade in trades {
            strategy.run(trade);
        }
    }
}
