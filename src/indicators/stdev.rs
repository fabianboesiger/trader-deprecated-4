use super::Ema;
use crate::Number;

#[derive(Copy, Clone)]
pub struct Stdev {
    var: Ema,
    mean: Ema,
}

impl Stdev {
    pub fn new(period: Number) -> Self {
        Self {
            var: Ema::new(period),
            mean: Ema::new(period),
        }
    }

    pub fn run(&mut self, input: Number) {
        self.mean.run(input);
        self.var.run((input - self.mean.get()).powi(2));
    }

    pub fn get(&self) -> Number {
        self.var.get().sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        let mut stdev = Stdev::new(3.0);
        stdev.run(0.0);
        assert_eq!(stdev.get(), 0.0);
        stdev.run(16.0);
        assert_eq!(stdev.get(), (32.0 as Number).sqrt());
    }
}
