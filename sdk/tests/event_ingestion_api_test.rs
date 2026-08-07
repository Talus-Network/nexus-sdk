#![cfg(feature = "events")]

use nexus_sdk::sui::{
    self,
    events::{EventQuery, RawEventQuery},
    traits::FieldMaskUtil as _,
};

#[test]
fn raw_event_query_preserves_the_request_and_event() {
    let filter = sui::grpc::EventFilter::default();
    let read_mask = sui::grpc::FieldMask::from_paths(["contents"]);
    let query = RawEventQuery::new(filter.clone(), read_mask.clone());
    let mut event = sui::grpc::Event::default();
    event.checkpoint = Some(7);
    event.transaction_digest = Some("digest".to_owned());
    event.event_index = Some(3);

    assert_eq!(query.filter(), filter);
    assert_eq!(query.read_mask(), read_mask);
    assert_eq!(query.decode(event.clone()).unwrap(), Some(event));
}
