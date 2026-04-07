//! Scoped collector for L2→L1 logs produced during EVM execution.
//!
//! Uses a thread-local internally, but access is scoped through [`LogCollector`]
//! which ensures logs are always drained at the right time and cannot leak
//! between unrelated EVM executions.
//!
//! Usage:
//! ```ignore
//! let mut collector = LogCollector::new();
//! for tx in transactions {
//!     collector.set_tx_number(tx_idx);
//!     evm.transact_commit(tx);  // precompile calls push_log()
//!     let logs = collector.take_tx_logs();
//! }
//! ```

use revm::primitives::{Address, B256};
use std::cell::RefCell;

/// Structured L2→L1 log entry matching the ZKsync OS protocol format.
#[derive(Debug, Clone)]
pub struct L2ToL1Log {
    pub l2_shard_id: u8,
    pub is_service: bool,
    pub tx_number_in_block: u16,
    pub sender: Address,
    pub key: B256,
    pub value: B256,
}

thread_local! {
    static L2_TO_L1_LOGS: RefCell<Vec<L2ToL1Log>> = RefCell::new(Vec::new());
    static CURRENT_TX_NUMBER: RefCell<u16> = RefCell::new(0);
}

/// Push a new L2→L1 log. Called by the L1Messenger precompile.
///
/// This is the only function the precompile should call. All other access
/// goes through [`LogCollector`].
pub fn push_log(sender: Address, key: B256, value: B256) {
    let tx_number = CURRENT_TX_NUMBER.with(|n| *n.borrow());
    L2_TO_L1_LOGS.with(|logs| {
        logs.borrow_mut().push(L2ToL1Log {
            l2_shard_id: 0,
            is_service: true,
            tx_number_in_block: tx_number,
            sender,
            key,
            value,
        });
    });
}

/// Scoped accessor for L2→L1 logs. Ensures logs are properly drained
/// and cannot leak between executions.
///
/// Create one per block execution. Drop clears any remaining logs.
pub struct LogCollector {
    _private: (), // prevent construction outside this module
}

impl LogCollector {
    /// Create a new collector, clearing any stale state.
    pub fn new() -> Self {
        L2_TO_L1_LOGS.with(|logs| logs.borrow_mut().clear());
        CURRENT_TX_NUMBER.with(|n| *n.borrow_mut() = 0);
        Self { _private: () }
    }

    /// Set the current transaction number. Call before each tx execution.
    pub fn set_tx_number(&mut self, tx_number: u16) {
        CURRENT_TX_NUMBER.with(|n| *n.borrow_mut() = tx_number);
    }

    /// Drain logs produced during the current transaction.
    /// Call after each `transact_commit`.
    pub fn take_tx_logs(&mut self) -> Vec<L2ToL1Log> {
        L2_TO_L1_LOGS.with(|logs| logs.borrow_mut().drain(..).collect())
    }
}

impl Drop for LogCollector {
    fn drop(&mut self) {
        // Ensure no logs leak to subsequent executions on this thread.
        L2_TO_L1_LOGS.with(|logs| logs.borrow_mut().clear());
    }
}
