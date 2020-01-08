use crate::consensus::{Step, VoteOn};
use std::cmp::Ordering;

pub struct VoteRegressionChecker {
    last_vote: Option<VoteOn>,
}

impl VoteRegressionChecker {
    pub fn new() -> VoteRegressionChecker {
        VoteRegressionChecker {
            last_vote: None,
        }
    }

    pub fn check(&mut self, vote_on: &VoteOn) -> bool {
        assert!(
            match vote_on.step.step {
                Step::Propose | Step::Prevote | Step::Precommit => true,
                _ => false,
            },
            "We don't vote on Commit. Check your code"
        );

        let monotonic = if let Some(last_vote) = &self.last_vote {
            match last_vote.step.cmp(&vote_on.step) {
                Ordering::Less => true,
                Ordering::Greater => false,
                Ordering::Equal => last_vote.block_hash == vote_on.block_hash,
            }
        } else {
            true
        };

        if monotonic {
            self.last_vote = Some(vote_on.clone());
        }
        monotonic
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::consensus::VoteStep;

    use primitives::H256;

    #[test]
    fn test_initial_set() {
        let mut checker = VoteRegressionChecker::new();

        let random_step = VoteStep::new(100, 10, Step::Prevote);
        let random_hash = Some(H256::random().into());
        assert!(checker.check(&VoteOn {
            step: random_step,
            block_hash: random_hash
        }))
    }

    #[test]
    #[should_panic]
    fn test_panic_on_commit() {
        let mut checker = VoteRegressionChecker::new();

        let random_commit_step = VoteStep::new(100, 10, Step::Commit);
        let random_hash = Some(H256::random().into());
        checker.check(&VoteOn {
            step: random_commit_step,
            block_hash: random_hash,
        });
    }

    #[test]
    fn test_allow_height_increase() {
        let mut checker = VoteRegressionChecker::new();

        checker.check(&VoteOn {
            step: VoteStep::new(100, 10, Step::Prevote),
            block_hash: Some(H256::from(1).into()),
        });

        assert!(checker.check(&VoteOn {
            step: VoteStep::new(101, 10, Step::Prevote),
            block_hash: Some(H256::from(2).into())
        }))
    }

    #[test]
    fn test_disallow_height_decrease() {
        let mut checker = VoteRegressionChecker::new();

        checker.check(&VoteOn {
            step: VoteStep::new(100, 10, Step::Prevote),
            block_hash: Some(H256::from(1).into()),
        });

        assert!(!checker.check(&VoteOn {
            step: VoteStep::new(99, 10, Step::Prevote),
            block_hash: Some(H256::from(2).into())
        }))
    }

    #[test]
    fn test_allow_view_increase() {
        let mut checker = VoteRegressionChecker::new();

        checker.check(&VoteOn {
            step: VoteStep::new(100, 10, Step::Prevote),
            block_hash: Some(H256::from(1).into()),
        });

        assert!(checker.check(&VoteOn {
            step: VoteStep::new(100, 11, Step::Prevote),
            block_hash: Some(H256::from(2).into())
        }))
    }

    #[test]
    fn test_disallow_view_decrease() {
        let mut checker = VoteRegressionChecker::new();

        checker.check(&VoteOn {
            step: VoteStep::new(100, 10, Step::Prevote),
            block_hash: Some(H256::from(1).into()),
        });

        assert!(!checker.check(&VoteOn {
            step: VoteStep::new(100, 9, Step::Prevote),
            block_hash: Some(H256::from(2).into())
        }))
    }

    #[test]
    fn test_allow_step_increased() {
        let mut checker = VoteRegressionChecker::new();

        checker.check(&VoteOn {
            step: VoteStep::new(100, 10, Step::Prevote),
            block_hash: Some(H256::from(1).into()),
        });

        assert!(checker.check(&VoteOn {
            step: VoteStep::new(100, 10, Step::Precommit),
            block_hash: Some(H256::from(2).into())
        }))
    }

    #[test]
    fn test_disallow_step_decreased() {
        let mut checker = VoteRegressionChecker::new();

        checker.check(&VoteOn {
            step: VoteStep::new(100, 10, Step::Prevote),
            block_hash: Some(H256::from(1).into()),
        });

        assert!(!checker.check(&VoteOn {
            step: VoteStep::new(100, 10, Step::Propose),
            block_hash: Some(H256::from(2).into())
        }))
    }

    #[test]
    fn test_allow_same_hash() {
        let mut checker = VoteRegressionChecker::new();

        let block_hash = Some(H256::random().into());
        checker.check(&VoteOn {
            step: VoteStep::new(100, 10, Step::Prevote),
            block_hash,
        });

        assert!(checker.check(&VoteOn {
            step: VoteStep::new(100, 10, Step::Prevote),
            block_hash,
        }))
    }

    #[test]
    fn test_disallow_hash_change() {
        let mut checker = VoteRegressionChecker::new();

        checker.check(&VoteOn {
            step: VoteStep::new(100, 10, Step::Prevote),
            block_hash: Some(H256::from(1).into()),
        });

        assert!(!checker.check(&VoteOn {
            step: VoteStep::new(100, 10, Step::Prevote),
            block_hash: Some(H256::from(2).into())
        }))
    }
}
