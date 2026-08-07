lazy_static::lazy_static! {
    pub(super) static ref STREAM_RECONNECTIONS: prometheus::Counter =
        prometheus::register_counter!(
            "poller_stream_reconnections",
            "Number of event stream reconnections"
        )
        .unwrap();

    pub(super) static ref REPLAY_REQUESTS: prometheus::Counter =
        prometheus::register_counter!(
            "poller_event_replay_requests",
            "Number of event replay requests"
        )
        .unwrap();

    pub(super) static ref FILTERED_EVENTS_RECEIVED: prometheus::Counter =
        prometheus::register_counter!(
            "poller_filtered_events_received",
            "Number of events received after the query filter"
        )
        .unwrap();

    pub(super) static ref EVENT_PARSE_DURATION: prometheus::Histogram =
        prometheus::register_histogram!(
            "poller_event_parse_duration",
            "Duration of query event decoding [s]",
            vec![0.00001, 0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05]
        )
        .unwrap();

    pub(super) static ref EVENTS_PER_PAGE: prometheus::Histogram =
        prometheus::register_histogram!(
            "poller_events_per_page",
            "Number of query events per event page",
            vec![0.0, 1.0]
        )
        .unwrap();

    pub(super) static ref EVENT_PAGE_CHANNEL_LEN: prometheus::Gauge =
        prometheus::register_gauge!(
            "poller_event_page_channel_len",
            "Number of event pages waiting for the consumer"
        )
        .unwrap();

    pub(super) static ref SEND_PAGE_BACKPRESSURE_DURATION: prometheus::Histogram =
        prometheus::register_histogram!(
            "poller_send_page_backpressure_duration",
            "Duration blocked while sending an event page [s]",
            vec![0.0001, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0, 10.0]
        )
        .unwrap();
}
