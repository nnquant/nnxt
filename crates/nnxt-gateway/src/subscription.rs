//! Subscription manager for instruments.

use nnxt_specs::market::InstrumentId;

#[derive(Debug, Default)]
pub struct SubscriptionManager {
    instruments: Vec<InstrumentId>,
}

impl SubscriptionManager {
    pub fn new() -> Self {
        Self { instruments: Vec::new() }
    }

    pub fn subscribe(&mut self, instrument_id: InstrumentId) -> bool {
        if self.instruments.iter().any(|id| id == &instrument_id) {
            return false;
        }
        self.instruments.push(instrument_id);
        true
    }

    pub fn unsubscribe(&mut self, instrument_id: &InstrumentId) -> bool {
        if let Some(index) = self.instruments.iter().position(|id| id == instrument_id) {
            self.instruments.swap_remove(index);
            return true;
        }
        false
    }

    pub fn is_subscribed(&self, instrument_id: &InstrumentId) -> bool {
        self.instruments.iter().any(|id| id == instrument_id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &InstrumentId> {
        self.instruments.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn subscribe_and_unsubscribe() {
        let mut manager = SubscriptionManager::new();
        let instrument = InstrumentId::from_str("IF2409").expect("instrument");
        assert!(manager.subscribe(instrument));
        assert!(!manager.subscribe(instrument));
        assert!(manager.is_subscribed(&instrument));
        assert!(manager.unsubscribe(&instrument));
        assert!(!manager.is_subscribed(&instrument));
    }
}
