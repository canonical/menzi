use futures_util::StreamExt;
use menzi_events::bus::NatsEventBus;
use menzi_events::Event;

fn nats_url() -> Option<String> {
    std::env::var("MENZI_TEST_NATS_URL").ok()
}

fn unique_suffix() -> String {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("{}_{}", std::process::id(), n)
}

#[tokio::test]
async fn event_bus_round_trips_events() {
    let Some(url) = nats_url() else {
        eprintln!("skipping: MENZI_TEST_NATS_URL not set");
        return;
    };
    let bus = NatsEventBus::connect(&url).await.expect("connect to nats");
    let suffix = unique_suffix();
    let subject = format!("menzi.it.{suffix}");
    let stream = format!("menzi_it_{suffix}");

    bus.ensure_stream(&stream, &[&subject])
        .await
        .expect("ensure stream");

    let sent = Event::new("preview.ready", "sess-1", "session", "payload-data");
    bus.publish(&subject, &sent).await.expect("publish event");

    let mut events = bus
        .subscribe::<String>(&stream, &subject)
        .await
        .expect("subscribe");
    let received = events
        .next()
        .await
        .expect("stream ended")
        .expect("message result");
    assert_eq!(received.event_type, "preview.ready");
    assert_eq!(received.aggregate_id, "sess-1");
    assert_eq!(received.payload, "payload-data");
}

#[tokio::test]
async fn event_bus_delivers_events_in_order() {
    let Some(url) = nats_url() else {
        eprintln!("skipping: MENZI_TEST_NATS_URL not set");
        return;
    };
    let bus = NatsEventBus::connect(&url).await.expect("connect to nats");
    let suffix = unique_suffix();
    let subject = format!("menzi.it.{suffix}");
    let stream = format!("menzi_it_{suffix}");

    bus.ensure_stream(&stream, &[&subject])
        .await
        .expect("ensure stream");

    for i in 0..3 {
        let sent = Event::new("env.status", format!("env-{i}"), "environment", i);
        bus.publish(&subject, &sent).await.expect("publish event");
    }

    let mut events = bus
        .subscribe::<i32>(&stream, &subject)
        .await
        .expect("subscribe");
    for i in 0..3 {
        let received = events
            .next()
            .await
            .expect("stream ended")
            .expect("message result");
        assert_eq!(received.aggregate_id, format!("env-{i}"));
        assert_eq!(received.payload, i);
    }
}

#[tokio::test]
async fn event_bus_ensure_stream_is_idempotent() {
    let Some(url) = nats_url() else {
        eprintln!("skipping: MENZI_TEST_NATS_URL not set");
        return;
    };
    let bus = NatsEventBus::connect(&url).await.expect("connect to nats");
    let suffix = unique_suffix();
    let subject = format!("menzi.it.{suffix}");
    let stream = format!("menzi_it_{suffix}");

    bus.ensure_stream(&stream, &[&subject])
        .await
        .expect("first ensure");
    bus.ensure_stream(&stream, &[&subject])
        .await
        .expect("second ensure is idempotent");
}
