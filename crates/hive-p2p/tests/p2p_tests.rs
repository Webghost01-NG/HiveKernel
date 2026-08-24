use hive_p2p::auction::{AuctionMatcher, CandidateBid};

#[test]
fn test_auction_matcher_scoring() {
    let bid_fast = CandidateBid {
        worker_id: "Worker_Fast".to_string(),
        bid_bounty: 100,
        estimated_duration_ms: 100,
        reputation_score: 90,
    };

    let bid_slow = CandidateBid {
        worker_id: "Worker_Slow".to_string(),
        bid_bounty: 100,
        estimated_duration_ms: 1000,
        reputation_score: 90,
    };

    let winning = AuctionMatcher::select_best_bid(&[bid_fast, bid_slow], 100).unwrap();
    assert_eq!(winning.worker_id, "Worker_Fast");
}
