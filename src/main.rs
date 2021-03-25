pub mod exchanges;
pub mod indicators;
pub mod strategies;

use exchanges::*;
use strategies::*;

type Number = f32;
type Market = String;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    pretty_env_logger::init();
    log::info!("Starting trader.");

    /*
    let mut strategy = Multi::new()
        .with(Simulated::new(Hold::new(), 0.001, 11))
        .with(Simulated::new(
            Duplicated::new(Interval::new(Custom::new(), 1000 * 60)),
            0.001,
            3,
        ));

    Historical::new(true).run(&mut strategy).await;
    println!("{}", strategy);
    */
    
    
    let markets = vec![
        "BTCUSDT", "ETHUSDT", "CHZUSDT", "BNBUSDT", "DOGEUSDT", "ADAUSDT", "BCHUSDT", "XRPUSDT",
        "LTCUSDT", "EOSUSDT", "DOTUSDT",
    ];
    Binance::new(markets, true)
        .await
        .run(&mut Duplicated::new(Interval::new(
            Random::new(),
            1000 * 60,
        )))
        .await;
}
