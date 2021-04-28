#![forbid(unstable_features)]
#![forbid(unsafe_code)]

mod error;
pub mod exchanges;
pub mod indicators;
pub mod loggers;
pub mod strategies;

pub use error::Error;

use exchanges::*;
use strategies::*;

use chrono::{TimeZone, Utc};

type Number = f32;
type Market = String;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    pretty_env_logger::init();

    log::info!("Starting trader.");

    #[allow(unused_mut)]
    let mut strategy = Duplicated::new(Interval::new(Custom::new(), 1000 * 60));
    let markets = vec![
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
        "LINKUSDT",

        "XMRUSDT",
        "XLMUSDT",
        "BTTUSDT",
        "TRXUSDT",
        "VETUSDT",
    ];

    #[cfg(not(feature = "live"))]
    {
        log::warn!("Trading in simulated environment.");

        let mut simulated = Multi::new()
            .with(Simulated::new(Hold::new(), 0.001, 13))
            .with(Simulated::new(strategy, 0.001, 1));

        Historical::new(
            &markets,
            Utc.ymd(2021, 4, 1).and_hms(0, 0, 0),
            Utc::now(),
            true
        )
            .run(&mut simulated)
            .await;

        println!("{}", simulated);

        #[cfg(feature = "plot")]
        simulated.plot();
    }

    #[cfg(feature = "live")]
    {
        use chrono::Duration;

        log::warn!("Trading on live exchange.");

        
        
        Historical::new(
            &markets,
            Utc.ymd(2021, 4, 1).and_hms(0, 0, 0),
            Utc::now(),
            false
        )
            .run(&mut strategy)
            .await;
        Binance::new(&markets, false).await.run(&mut strategy).await;
    }
}
