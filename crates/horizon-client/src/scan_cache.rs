use std::time::Instant;

pub struct ScanCache<T> {
    revision: Option<u64>,
    fade: f32,
    changes_at: Option<Instant>,
    items: Vec<T>,
}

impl<T> Default for ScanCache<T> {
    fn default() -> Self {
        Self {
            revision: None,
            fade: 0.0,
            changes_at: None,
            items: Vec::new(),
        }
    }
}

impl<T> ScanCache<T> {
    pub fn refresh(
        &mut self,
        revision: u64,
        fade: f32,
        now: Instant,
        scan: impl FnOnce() -> (Vec<T>, Option<Instant>),
    ) -> &[T] {
        let reusable = self.revision == Some(revision)
            && self.fade == fade
            && self.changes_at.is_none_or(|at| now < at);
        if !reusable {
            (self.items, self.changes_at) = scan();
            self.revision = Some(revision);
            self.fade = fade;
        }
        &self.items
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn a_settled_scan_is_reused_until_the_revision_changes() {
        let mut cache = ScanCache::default();
        let t0 = Instant::now();
        let mut scans = 0;
        let mut get = |cache: &mut ScanCache<u32>, rev, fade, now| {
            cache
                .refresh(rev, fade, now, || {
                    scans += 1;
                    (vec![scans], None)
                })
                .to_vec()
        };
        assert_eq!(get(&mut cache, 1, 0.0, t0), vec![1]);
        assert_eq!(
            get(&mut cache, 1, 0.0, t0 + Duration::from_secs(1)),
            vec![1]
        );
        assert_eq!(
            get(&mut cache, 2, 0.0, t0 + Duration::from_secs(2)),
            vec![2]
        );
        assert_eq!(
            get(&mut cache, 2, 0.5, t0 + Duration::from_secs(3)),
            vec![3]
        );
    }

    #[test]
    fn a_scan_is_repeated_once_a_pending_item_may_have_appeared() {
        let mut cache = ScanCache::default();
        let t0 = Instant::now();
        let mut scans = 0;
        let mut get = |cache: &mut ScanCache<u32>, now| {
            cache
                .refresh(7, 0.75, now, || {
                    scans += 1;
                    (vec![scans], Some(t0 + Duration::from_millis(300)))
                })
                .to_vec()
        };
        assert_eq!(get(&mut cache, t0), vec![1]);
        assert_eq!(get(&mut cache, t0 + Duration::from_millis(100)), vec![1]);
        assert_eq!(get(&mut cache, t0 + Duration::from_millis(300)), vec![2]);
    }
}
