use graphql_client::GraphQLQuery;

type SuiAddress = sui_sdk_types::Address;
type UInt53 = u64;
type JSON = serde_json::Value;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/events/graphql/schema-1.61.2.graphql",
    query_path = "src/events/graphql/events_query.graphql",
    response_derives = "Debug"
)]
pub struct EventsQuery;
