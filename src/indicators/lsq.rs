use super::Ema;
use crate::Number;

#[derive(Copy, Clone)]
pub struct Lsq {
    aa: Ema,
    ab: Ema,
    lsq: Number,
}

impl Lsq {
    pub fn new(period: Number) -> Self {
        Self {
            aa: Ema::new(period),
            ab: Ema::new(period),
            lsq: 0.0,
        }
    }

    pub fn run(&mut self, a: Number, b: Number) {
        self.aa.run(a.powi(2));
        self.ab.run(a * b);
        self.lsq = (1.0 / self.aa.get()) * self.ab.get();
    }

    pub fn get(&self) -> Number {
        self.lsq
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        let mut lsq = Lsq::new(1.0);
        lsq.run(2.0, 4.0);
        assert_eq!(lsq.get(), 2.0);
        lsq.run(-8.0, 4.0);
        assert_eq!(lsq.get(), -0.5);
    }
}
