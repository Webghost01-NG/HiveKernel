use hive_core::{error::HiveError, error::Result, types::AgentId};

#[derive(Debug, Clone)]
pub struct CandidateBid {
    pub worker_id: AgentId,
    pub bid_bounty: u64,
    pub estimated_duration_ms: u64,
    pub reputation_score: u32,
}

/// Reverse-Auction Matcher: Scores bids based on reputation, price efficiency, and latency
pub struct AuctionMatcher;

impl AuctionMatcher {
    pub fn select_best_bid(bids: &[CandidateBid], max_bounty: u64) -> Result<CandidateBid> {
        if bids.is_empty() {
            return Err(HiveError::TaskExecutionError("No bids received for RFQ".to_string()));
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
        let price_factor = if bid.bid_bounty > 0 {
            (max_bounty * 100) / bid.bid_bounty
        } else {
            100
        };
        let rep_factor = bid.reputation_score as u64;
        let latency_penalty = (bid.estimated_duration_ms / 100).max(1);

        (rep_factor * price_factor) / latency_penalty
    }
}
