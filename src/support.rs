use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_SUFFIX_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) fn unix_epoch_nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}

pub(crate) fn temp_suffix() -> String {
    let counter = TEMP_SUFFIX_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{}-{}-{counter}", std::process::id(), unix_epoch_nanos())
}

pub(crate) fn python_available() -> bool {
    let python = std::env::var("PYTHON").unwrap_or_else(|_| "python".to_string());
    std::process::Command::new(python)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}
