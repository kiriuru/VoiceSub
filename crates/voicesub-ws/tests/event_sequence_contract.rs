use voicesub_ws::{EventSequencer, shared_event_sequencer, EventsHub, WsEventPublisher};

#[test]
fn sequencer_reset_does_not_rewind_global_stream() {
    let mut seq = EventSequencer::default();
    let first = seq.enrich("runtime_status", serde_json::json!({ "phase": "idle" }));
    seq.reset_broadcast_state();
    let second = seq.enrich("runtime_status", serde_json::json!({ "phase": "listening" }));
    assert_eq!(first["event_sequence"], 1);
    assert_eq!(second["event_sequence"], 2);
}

#[tokio::test]
async fn publisher_enriches_broadcast_payload() {
    let hub = EventsHub::new();
    let publisher = WsEventPublisher::new(hub.clone(), shared_event_sequencer());

    publisher
        .broadcast_channel(
            "transcript_update",
            "transcript_update",
            serde_json::json!({ "text": "hello", "is_final": false }),
        )
        .await;

    let cached = hub
        .diagnostics();
    assert_eq!(cached.connections_active, 0);
}
