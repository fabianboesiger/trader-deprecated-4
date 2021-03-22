use super::{Cum, Ema, Lsq};
use crate::Number;

#[derive(Copy, Clone)]
pub struct Val {
    mov: Cum,
    mov_ema: Ema,
    price_ema: Ema,
    marginal_price: Lsq,
    val: Number,
}

impl Val {
    pub fn new(offset_period: Number, marginal_price_period: Number) -> Self {
        Self {
            mov: Cum::new(),
            mov_ema: Ema::new(offset_period),
            price_ema: Ema::new(offset_period),
            marginal_price: Lsq::new(marginal_price_period),
            val: 0.0,
        }
    }

    pub fn run(&mut self, quantity: Number, price: Number) {
        self.mov.run(quantity);
        self.mov_ema.run(self.mov.get());
        self.price_ema.run(price);

        let mov_aligned = self.mov.get() - self.mov_ema.get();
        let price_aligned = price - self.price_ema.get();
        self.marginal_price.run(mov_aligned, price_aligned);

        if !self.marginal_price.get().is_nan() {
            self.val = self.price_ema.get() + mov_aligned * self.marginal_price.get();
        } else {
            self.val = self.price_ema.get()
        }
    }

    pub fn get(&self) -> Number {
        self.val
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_0() {
        let mut val = Val::new(3.0, 1.0);
        val.run(0.0, 0.0);
        assert_eq!(val.get(), 0.0);
        val.run(8.0, 16.0);
        assert_eq!(val.get(), 16.0);
    }
}
