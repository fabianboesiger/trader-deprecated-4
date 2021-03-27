use crate::Number;

#[derive(Copy, Clone)]
pub struct Ema {
    output: Option<Number>,
    alpha: Number,
}

impl Ema {
    pub fn new(period: Number) -> Self {
        debug_assert!(period >= 1.0);

        Self {
            alpha: 2.0 / (1.0 + period),
            output: None,
        }
    }

    pub fn run(&mut self, input: Number) {
        if let Some(output) = &mut self.output {
            *output = input * self.alpha + *output * (1.0 - self.alpha);
        } else {
            self.output = Some(input);
        }
    }

    pub fn get(&self) -> Number {
        self.output.expect("No value assigned to EMA.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        let mut ema = Ema::new(3.0);
        ema.run(0.0);
        assert_eq!(ema.get(), 0.0);
        ema.run(2.0);
        assert_eq!(ema.get(), 1.0);
        ema.run(-1.0);
        assert_eq!(ema.get(), 0.0);
        ema.run(16.0);
        assert_eq!(ema.get(), 8.0);
    }
}
