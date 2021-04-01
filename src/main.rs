pub mod exchanges;
pub mod indicators;
pub mod strategies;
mod error;

pub use error::Error;

use exchanges::*;
use strategies::*;

type Number = f32;
type Market = String;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    pretty_env_logger::init();
    log::info!("Starting trader.");

    let mut strategy = Duplicated::new(Interval::new(Custom::new(), 1000 * 60));

    let mut simulated = Multi::new()
        .with(Simulated::new(Hold::new(), 0.001, 11))
        .with(Simulated::new(strategy, 0.001, 2));

    Historical::new(true).run(&mut simulated).await;
    println!("{}", simulated);

    /* 
    let markets = vec![
        "BTCUSDT", "ETHUSDT", "CHZUSDT", "BNBUSDT", "DOGEUSDT", "ADAUSDT", "BCHUSDT", "XRPUSDT",
        "LTCUSDT", "EOSUSDT", "DOTUSDT",
    ];
    Binance::new(markets, false)
        .await
        .run(&mut Duplicated::new(Interval::new(
            Random::new(),
            1000 * 60,
        )))
        .await;
    */
    
    /* 
    let markets = vec![
        "BTCUSDT", "ETHUSDT", "CHZUSDT", "BNBUSDT", "DOGEUSDT", "ADAUSDT", "BCHUSDT", "XRPUSDT",
        "LTCUSDT", "EOSUSDT", "DOTUSDT", /*"THETAUSDT", "LINKUSDT"*/
    ];

    Binance::new(markets, false)
        .await
        .run(&mut strategy)
        .await;*/
}
