use crate::Number;

#[derive(Copy, Clone)]
pub struct Cum {
    sum: Number,
}

impl Cum {
    pub fn new() -> Self {
        Self { sum: 0.0 }
    }

    pub fn run(&mut self, input: Number) {
        self.sum += input;
    }

    pub fn get(&self) -> Number {
        self.sum
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_0() {
        let mut cum = Cum::new();
        cum.run(10.0);
        assert_eq!(cum.get(), 10.0);
        cum.run(40.0);
        assert_eq!(cum.get(), 50.0);
        cum.run(-100.0);
        assert_eq!(cum.get(), -50.0);
    }
}
