use rand::Rng;

fn compute_token_value(fixed: u64, lower: u64, upper: u64) -> u64 {
    let mut rng = rand::rng();
    if fixed == 0 {
        rng.random_range(lower..=upper)
    } else {
        fixed
    }
}

pub fn compute_token_values(fixed: u64, lower: u64, upper: u64, count: u64) -> Vec<u64> {
    let mut values: Vec<u64> = Vec::new();
    for _ in 0..count {
        values.push(compute_token_value(fixed, lower, upper));
    }
    values
}

pub fn compute_sum_total(tokens: &Vec<u64>) -> u64 {
    let mut sum: u64 = 0;
    for t in tokens {
        sum += t;
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixed_amount() {
        assert_eq!(compute_token_value(5000, 100, 1000), 5000);
    }

    #[test]
    fn test_variable_amount() {
        let result = compute_token_value(0, 100, 1000);
        assert!(result >= 100 && result <= 1000);
    }
}
