use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::mpsc;

use api::diagnostics::Field;
use tracing_subscriber::Registry;
use tracing_subscriber::layer::SubscriberExt;

use super::Recorder;

#[test]
fn info_events_keep_their_message_and_numbers() {
    let (sender, receiver) = mpsc::sync_channel(8);
    let recorder = Recorder {
        sender,
        stopping: Arc::new(AtomicBool::new(false)),
        sequence: AtomicU64::new(0),
        dropped: Arc::new(AtomicU64::new(0)),
        run_id: "test".into(),
        version: "0.0.0".into(),
    };
    tracing::subscriber::with_default(Registry::default().with(recorder), || {
        tracing::info!(
            event_code = "attachments_pruned",
            count = 2_u64,
            "pruned the attachments of 2 closed items"
        );
    });
    let record = receiver.try_recv().unwrap();
    assert_eq!(
        record.detail.as_deref(),
        Some("pruned the attachments of 2 closed items")
    );
    assert_eq!(record.fields["count"], Field::Number(2));
    assert_eq!(
        serde_json::to_value(&record.fields).unwrap(),
        serde_json::json!({"count": 2, "event_code": "attachments_pruned"})
    );
}
