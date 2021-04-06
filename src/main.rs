mod error;
pub mod exchanges;
pub mod indicators;
pub mod strategies;

pub use error::Error;

use exchanges::*;
use strategies::*;

use chrono::{Duration, Utc, TimeZone};

type Number = f32;
type Market = String;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    pretty_env_logger::init();
    log::info!("Starting trader.");

    let mut strategy = Duplicated::new(Interval::new(Custom::new(), 1000 * 60));

    #[cfg(not(feature = "live"))]
    {
        let mut simulated = Multi::new()
            .with(Simulated::new(Hold::new(), 0.001, 13))
            .with(Simulated::new(strategy, 0.001, 2));

        Historical::new(
            Utc.ymd(2021, 3, 1).and_hms(0, 0, 0), 
            Utc::now(), 
            true
        ).run(&mut simulated).await;
        println!("{}", simulated);
    }
    
    #[cfg(feature = "live")]
    {
        let markets = vec![
            "BTCUSDT", "ETHUSDT", "CHZUSDT", "BNBUSDT", "DOGEUSDT", "ADAUSDT", "BCHUSDT", "XRPUSDT",
            "LTCUSDT", "EOSUSDT", "DOTUSDT", "THETAUSDT", "LINKUSDT"
        ];

        Historical::new(Utc::now() - Duration::days(1), Utc::now(), false)
            .run(&mut strategy)
            .await;
        Binance::new(markets, false).await.run(&mut strategy).await;
    }
}
