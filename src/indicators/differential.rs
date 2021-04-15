use super::Macd;
use crate::Number;

#[derive(Copy, Clone)]
pub struct Differential {
    period: Number,
    macd: Macd,
}

impl Differential {
    pub fn new(period: Number) -> Self {
        Self {
            period,
            macd: Macd::new(period / 2.0, period, period / 2.0),
        }
    }

    pub fn run(&mut self, input: Number) {
        self.macd.run(input);
    }

    pub fn first_derivative(&self) -> Number {
        self.macd.get() / (self.period / 4.0)
    }

    pub fn second_derivative(&self) -> Number {
        self.macd.get_hist() / (self.period / 4.0).powi(2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_derivation() {
        let mut diff = Differential::new(256.0);

        for i in 0..10000 {
            let x = i as Number * 0.5;
            diff.run(x);
        }

        assert_eq!(diff.first_derivative(), 0.49996948 as Number);
        assert_eq!(diff.second_derivative(), 0.0000000037252903 as Number);
    }

    #[test]
    fn second_derivation() {
        let mut diff = Differential::new(256.0);

        for i in 0i32..10000i32 {
            let x = i.pow(2) as Number;
            diff.run(x);
        }

        assert_eq!(diff.second_derivative(), 1.9862366 as Number);
    }
}
