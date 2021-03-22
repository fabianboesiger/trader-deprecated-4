use super::Ema;
use crate::Number;

#[derive(Copy, Clone)]
pub struct Macd {
    fast: Ema,
    slow: Ema,
    signal: Ema,
}

impl Macd {
    pub fn new(fast_period: Number, slow_period: Number, signal_period: Number) -> Self {
        Self {
            fast: Ema::new(fast_period),
            slow: Ema::new(slow_period),
            signal: Ema::new(signal_period),
        }
    }

    pub fn run(&mut self, input: Number) {
        self.fast.run(input);
        self.slow.run(input);
        self.signal.run(self.get());
    }

    pub fn get(&self) -> Number {
        self.fast.get() - self.slow.get()
    }

    pub fn get_signal(&self) -> Number {
        self.signal.get()
    }

    pub fn get_hist(&self) -> Number {
        self.get() - self.get_signal()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_0() {
        let mut macd = Macd::new(1.0, 3.0, 3.0);
        macd.run(0.0);
        assert_eq!(macd.get(), 0.0);
        assert_eq!(macd.get_signal(), 0.0);
        assert_eq!(macd.get_hist(), 0.0);
        macd.run(16.0);
        assert_eq!(macd.get(), 8.0);
        assert_eq!(macd.get_signal(), 4.0);
        assert_eq!(macd.get_hist(), 4.0);
    }
}
