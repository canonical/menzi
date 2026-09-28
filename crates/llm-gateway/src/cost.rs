pub struct CostCalculator;

impl CostCalculator {
    pub fn calculate(&self, input_tokens: i32, output_tokens: i32, model: &str) -> f64 {
        let (input_price, output_price) = match model {
            "gpt-4" => (0.03, 0.06),
            "gpt-3.5-turbo" => (0.0005, 0.0015),
            "claude-sonnet-5" => (0.003, 0.015),
            _ => (0.001, 0.002),
        };
        (input_tokens as f64 * input_price / 1000.0)
            + (output_tokens as f64 * output_price / 1000.0)
    }

    pub fn estimate_tokens(&self, text: &str) -> i32 {
        (text.len() / 4) as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cost_calculates_gpt4() {
        let calc = CostCalculator;
        let cost = calc.calculate(1000, 500, "gpt-4");
        assert!(cost > 0.0);
    }

    #[test]
    fn cost_calculates_claude() {
        let calc = CostCalculator;
        let cost = calc.calculate(1000, 500, "claude-sonnet-5");
        assert!(cost > 0.0);
    }

    #[test]
    fn cost_uses_default_for_unknown_model() {
        let calc = CostCalculator;
        let cost = calc.calculate(1000, 500, "unknown-model");
        assert!(cost > 0.0);
    }

    #[test]
    fn estimate_tokens_returns_positive() {
        let calc = CostCalculator;
        assert!(calc.estimate_tokens("Hello world") > 0);
    }

    #[test]
    fn estimate_tokens_scales_with_length() {
        let calc = CostCalculator;
        let short = calc.estimate_tokens("Hi");
        let long = calc.estimate_tokens("This is a much longer text with many more tokens");
        assert!(long > short);
    }
}
