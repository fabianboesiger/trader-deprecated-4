use super::Strategy;
use crate::{
    exchanges::{Order, Side, Trade},
    indicators::*,
    Number,
};
use std::fmt;

#[derive(Clone, Copy)]
struct Data {
    timestamp: i64,
    price: Number,
    val: Number,
}

#[derive(Clone)]
pub struct Custom {
    val: Val,
    diff: Ema,
    diff_stdev: Stdev,
    hist_stdev: Stdev,
    macd: Macd,
    macd_long: Macd,
    stdev: Stdev,
    was_undervalued: bool,
    bought_at: i64,

    data: Vec<Data>,
    market: Option<String>,
    order: Vec<Order>,
}

impl Custom {
    pub fn new() -> Self {
        Self {
            val: Val::new(20000.0, 20000.0),
            diff: Ema::new(200.0),
            macd: Macd::new(100.0, 200.0, 50.0),
            macd_long: Macd::new(1000.0, 2000.0, 1.0),
            diff_stdev: Stdev::new(2000.0),
            hist_stdev: Stdev::new(2000.0),
            stdev: Stdev::new(2000.0),
            was_undervalued: false,
            bought_at: 0,

            data: Vec::new(),
            market: None,
            order: Vec::new(),
        }
    }
}

impl Strategy for Custom {
    fn run(
        &mut self,
        Trade {
            market,
            quantity,
            price,
            timestamp,
        }: Trade,
    ) -> Option<Order> {
        self.stdev.run(price);
        self.val.run(quantity, price);
        self.diff.run(price - self.val.get());
        self.diff_stdev.run(self.diff.get());
        self.macd.run(price);
        self.hist_stdev.run(self.macd.get_hist());
        self.macd_long.run(price);

        let max_stdev = self.stdev.get().min(price * 0.05);
        let trend = self.macd_long.get();
        let momentum = self.macd.get_hist();
        let is_undervalued = self.diff.get() < -self.diff_stdev.get() * 1.8;
        let mean_reversal = !is_undervalued && self.was_undervalued;
        let worth_it = 1.4 * max_stdev > price * 0.01;
        let has_momentum = momentum > 0.0;
        let is_bullish = trend > 0.0;
        let no_backoff = self.bought_at + 1000 * 60 * 60 * 12 < timestamp;

        #[cfg(feature = "plot")]
        {
            self.market = Some(market.clone());
            self.data.push(Data {
                price,
                timestamp,
                val: self.val.get(),
            });
        }

        /*
        log::trace!(
            "Running strategy: market={}, price={}, price_stdev={}, trend={}, momentum={}, diff={}, diff_stdev={}",
            market,
            price,
            self.stdev.get(),
            trend,
            momentum,
            self.diff.get(),
            self.diff_stdev.get(),
        );
        */

        let action = if
            mean_reversal &&
            worth_it &&
            //has_momentum &&
            is_bullish &&
            no_backoff
        {
            self.bought_at = timestamp;
            Some(Order {
                market,
                price,
                take_profit: Some(price + 1.4 * max_stdev),
                stop_loss: Some(price - 1.2 * max_stdev),
                side: Side::Buy,
            })
        } else {
            None
        };

        self.was_undervalued = is_undervalued;

        action
    }

    #[cfg(feature = "plot")]
    fn plot(&self) {
        log::info!("Generating plot ...");

        use plotters::prelude::*;

        let path = format!("plots/plot-{}.png", self.market.clone().unwrap());
        let root = BitMapBackend::new(&path, (1024, 768)).into_drawing_area();
        root.fill(&WHITE).unwrap();

        let mut chart = ChartBuilder::on(&root)
            .x_label_area_size(40)
            .y_label_area_size(40)
            .caption(
                format!("Analysis {}", self.market.clone().unwrap()),
                ("sans-serif", 50.0).into_font(),
            )
            .build_cartesian_2d(
                self.data.first().unwrap().timestamp..self.data.last().unwrap().timestamp,
                self.data
                    .iter()
                    .map(|&data| data.price)
                    .min_by(|a, b| a.partial_cmp(b).unwrap())
                    .unwrap()
                    ..self
                        .data
                        .iter()
                        .map(|&data| data.price)
                        .max_by(|a, b| a.partial_cmp(b).unwrap())
                        .unwrap(),
            )
            .unwrap();

        chart
            .configure_mesh()
            .light_line_style(&WHITE)
            .draw()
            .unwrap();

        chart
            .draw_series(LineSeries::new(
                self.data.iter().map(
                    |&Data {
                         timestamp, price, ..
                     }| (timestamp, price),
                ),
                &RED,
            ))
            .unwrap()
            .label("price");

        chart
            .draw_series(LineSeries::new(
                self.data
                    .iter()
                    .map(|&Data { timestamp, val, .. }| (timestamp, val)),
                &BLUE,
            ))
            .unwrap()
            .label("value");
    }
}

impl fmt::Display for Custom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "custom")
    }
}
