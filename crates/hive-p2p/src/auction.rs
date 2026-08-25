use hive_core::{error::HiveError, error::Result, registry::AgentRegistry, types::AgentId};

#[derive(Debug, Clone)]
pub struct CandidateBid {
    pub worker_id: AgentId,
    pub bid_bounty: u64,
    pub estimated_duration_ms: u64,
    pub reputation_score: u32,
}

impl CandidateBid {
    /// Dynamically constructs a bid based on live registry reputation and measured network latency
    pub fn create_dynamic_bid(
        worker_id: impl Into<AgentId>,
        max_bounty: u64,
        discount_percent: u64,
        measured_latency_ms: u64,
        registry: &AgentRegistry,
    ) -> Self {
        let id = worker_id.into();
        let reputation_score = registry.get_reputation(&id);
        let discount = (max_bounty * discount_percent.min(50)) / 100;
        let bid_bounty = (max_bounty.saturating_sub(discount)).max(1);

        Self {
            worker_id: id,
            bid_bounty,
            estimated_duration_ms: measured_latency_ms,
            reputation_score,
        }
    }
}

/// Reverse-Auction Matcher: Scores bids dynamically based on reputation, price efficiency, and measured latency
pub struct AuctionMatcher;

impl AuctionMatcher {
    pub fn select_best_bid(bids: &[CandidateBid], max_bounty: u64) -> Result<CandidateBid> {
        if bids.is_empty() {
            return Err(HiveError::TaskExecutionError(
                "No bids received for RFQ".to_string(),
            ));
        }

        let mut ranked = bids.to_vec();
        ranked.sort_by(|a, b| {
            let score_a = Self::compute_score(a, max_bounty);
            let score_b = Self::compute_score(b, max_bounty);
            score_b.cmp(&score_a)
        });

        Ok(ranked[0].clone())
    }

    fn compute_score(bid: &CandidateBid, max_bounty: u64) -> u64 {
        let price_factor = (max_bounty * 100)
            .checked_div(bid.bid_bounty)
            .unwrap_or(100);
        let rep_factor = bid.reputation_score as u64;
        let latency_penalty = (bid.estimated_duration_ms / 50).max(1);

        (rep_factor * price_factor) / latency_penalty
    }
}
