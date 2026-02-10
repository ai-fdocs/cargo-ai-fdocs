#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncStatus {
    Synced,
    SyncedFallback,
    Missing,
    Outdated,
    Corrupted,
}

pub fn is_healthy(statuses: &[SyncStatus]) -> bool {
    statuses
        .iter()
        .all(|status| matches!(status, SyncStatus::Synced | SyncStatus::SyncedFallback))
}

pub fn exit_code(statuses: &[SyncStatus]) -> i32 {
    if is_healthy(statuses) {
        0
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::{is_healthy, SyncStatus};

    #[test]
    fn healthy_when_all_statuses_are_synced() {
        let statuses = [
            SyncStatus::Synced,
            SyncStatus::SyncedFallback,
            SyncStatus::Synced,
        ];

        assert!(is_healthy(&statuses));
    }

    #[test]
    fn unhealthy_when_missing_exists() {
        let statuses = [SyncStatus::Synced, SyncStatus::Missing];

        assert!(!is_healthy(&statuses));
    }

    #[test]
    fn unhealthy_when_outdated_exists() {
        let statuses = [SyncStatus::SyncedFallback, SyncStatus::Outdated];

        assert!(!is_healthy(&statuses));
    }

    #[test]
    fn unhealthy_when_corrupted_exists() {
        let statuses = [SyncStatus::Corrupted];

        assert!(!is_healthy(&statuses));
    }
}
