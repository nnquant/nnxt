//! Queue naming helpers.

pub fn queue_path(data_type: &str, source: &str, target: &str) -> String {
    format!("{}/{}/{}", data_type, source, target)
}

pub fn broadcast_queue(data_type: &str, source: &str) -> String {
    queue_path(data_type, source, "public")
}

pub fn action_queue(strategy_id: &str, trade_gateway_id: &str) -> String {
    queue_path("action", strategy_id, trade_gateway_id)
}
