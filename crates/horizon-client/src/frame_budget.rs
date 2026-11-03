use std::time::{Duration, Instant};

pub struct FrameBudget {
    limit: Duration,
    spent: Duration,
}

impl FrameBudget {
    pub const fn new(limit: Duration) -> Self {
        Self {
            limit,
            spent: Duration::ZERO,
        }
    }

    pub fn left(&self) -> bool {
        self.spent < self.limit
    }

    pub fn charge(&mut self, spent: Duration) {
        self.spent += spent;
    }

    pub fn time<T>(&mut self, f: impl FnOnce() -> T) -> T {
        let started = Instant::now();
        let out = f();
        self.charge(started.elapsed());
        out
    }
}

pub fn take_batch<T>(pending: &mut Vec<T>, size: usize) -> Vec<T> {
    let at = pending.len().saturating_sub(size);
    pending.split_off(at)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_runs_out_once_charged_past_its_limit() {
        let mut b = FrameBudget::new(Duration::from_millis(2));
        assert!(b.left());
        b.charge(Duration::from_millis(1));
        assert!(b.left());
        b.charge(Duration::from_millis(1));
        assert!(!b.left());
    }

    #[test]
    fn a_zero_budget_is_spent_from_the_start() {
        assert!(!FrameBudget::new(Duration::ZERO).left());
    }

    #[test]
    fn timed_work_is_charged() {
        let mut b = FrameBudget::new(Duration::from_millis(5));
        let v = b.time(|| {
            std::thread::sleep(Duration::from_millis(8));
            7
        });
        assert_eq!(v, 7);
        assert!(!b.left());
    }

    #[test]
    fn batches_take_from_the_end_and_leave_the_rest() {
        let mut p = vec![1, 2, 3, 4, 5];
        assert_eq!(take_batch(&mut p, 2), vec![4, 5]);
        assert_eq!(p, vec![1, 2, 3]);
        assert_eq!(take_batch(&mut p, 10), vec![1, 2, 3]);
        assert!(p.is_empty());
        assert!(take_batch(&mut p, 3).is_empty());
    }
}
