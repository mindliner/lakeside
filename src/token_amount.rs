use rand::RngExt;

#[derive(Clone, Copy, Debug)]
pub enum AmountStrategy {
    Fixed(u64),
    Range { lower: u64, upper: u64 },
}

fn compute_token_value(strategy: AmountStrategy) -> u64 {
    match strategy {
        AmountStrategy::Fixed(value) => value,
        AmountStrategy::Range { lower, upper } => {
            let mut rng = rand::rng();
            if lower == upper {
                lower
            } else {
                rng.random_range(lower..=upper)
            }
        }
    }
}

/// Compute a value for each token to be generated. Values are either fixed or random
/// within a range.
pub fn compute_token_values(strategy: AmountStrategy, count: u64) -> Vec<u64> {
    let mut values: Vec<u64> = Vec::new();
    for _ in 0..count {
        values.push(compute_token_value(strategy));
    }
    values
}

/// Returns the sum of the amounts of all tokens generated
pub fn compute_sum_total(tokens: &Vec<u64>) -> u64 {
    tokens.iter().copied().sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixed_amount() {
        let strategy = AmountStrategy::Fixed(5000);
        assert_eq!(compute_token_value(strategy), 5000);
    }

    #[test]
    fn test_variable_amount() {
        let strategy = AmountStrategy::Range {
            lower: 100,
            upper: 1000,
        };
        let result = compute_token_value(strategy);
        assert!(result >= 100 && result <= 1000);
    }

    #[test]
    fn test_compute_sum() {
        let values = vec![21, 13, 14, 15, 16, 121];
        let sum = compute_sum_total(&values);
        assert_eq!(sum, 200);
    }
}
