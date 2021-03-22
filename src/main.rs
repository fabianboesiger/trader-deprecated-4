pub mod exchanges;
pub mod indicators;
pub mod strategies;

use exchanges::*;
use strategies::*;

type Number = f32;
type Market = String;

#[tokio::main]
async fn main() {
    let mut strategy = Multi::new()
        .with(Simulated::new(Hold::new(), 0.001, 11))
        .with(Simulated::new(
            Duplicated::new(Interval::new(Custom::new(), 1000 * 60)),
            0.001,
            1,
        ));

    Historical::new(true).run(&mut strategy).await;
    //Binance::new(true).await.run(&mut Duplicated::new(Interval::new(Custom::new(), 1000 * 60))).await;

    println!("{}", strategy);
}
