//! Thread-local collector for L2→L1 logs produced during EVM execution.
//!
//! The L1Messenger precompile (0x8008) calls [`push_log`] for each `sendToL1()`
//! call. The executor calls [`take_logs`] after each transaction to collect
//! the structured L2→L1 log entries produced during that transaction.
//!
//! This avoids reconstructing logs from EVM events post-hoc — the precompile
//! records the exact structured data at the point of emission.

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

/// Set the current transaction number within the block.
/// Call before executing each transaction.
pub fn set_tx_number(tx_number: u16) {
    CURRENT_TX_NUMBER.with(|n| *n.borrow_mut() = tx_number);
}

/// Get the current transaction number.
pub fn get_tx_number() -> u16 {
    CURRENT_TX_NUMBER.with(|n| *n.borrow())
}

/// Push a new L2→L1 log. Called by the L1Messenger precompile.
pub fn push_log(sender: Address, key: B256, value: B256) {
    let tx_number = get_tx_number();
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

/// Take all collected L2→L1 logs, clearing the buffer.
/// Call after each transaction or at block boundaries.
pub fn take_logs() -> Vec<L2ToL1Log> {
    L2_TO_L1_LOGS.with(|logs| logs.borrow_mut().drain(..).collect())
}

/// Clear all collected logs without returning them.
pub fn clear_logs() {
    L2_TO_L1_LOGS.with(|logs| logs.borrow_mut().clear());
}
