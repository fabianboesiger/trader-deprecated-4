use super::{Ema, Stdev};
use crate::Number;

#[derive(Copy, Clone)]
pub struct Bb {
    ma: Ema,
    sigma: Stdev,
    times: Number,
}

impl Bb {
    pub fn new(period: Number, times: Number) -> Self {
        Self {
            ma: Ema::new(period),
            sigma: Stdev::new(period),
            times,
        }
    }

    pub fn run(&mut self, input: Number) {
        self.ma.run(input);
        self.sigma.run(input);
    }

    pub fn get_ma(&self) -> Number {
        self.ma.get()
    }

    pub fn get_upper(&self) -> Number {
        self.get_ma() + self.times * self.sigma.get()
    }

    pub fn get_lower(&self) -> Number {
        self.get_ma() - self.times * self.sigma.get()
    }

    pub fn range(&self) -> Number {
        self.get_upper() - self.get_lower()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_0() {
        let mut bb = Bb::new(3.0, 1.0);
        bb.run(0.0);
        assert_eq!(bb.get_ma(), 0.0);
        assert_eq!(bb.get_upper(), 0.0);
        bb.run(16.0);
        assert_eq!(bb.get_ma(), 8.0);
        assert_eq!(bb.get_upper(), 8.0 + (8.0 as Number).sqrt());
    }
}