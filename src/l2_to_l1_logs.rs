//! L2→L1 log collection via the REVM `chain` context field.
//!
//! The REVM `Context` has a generic `CHAIN` parameter (default `()`) for
//! chain-specific data. We use it to store L2→L1 logs produced by the
//! L1Messenger precompile during execution.
//!
//! This avoids thread-locals entirely — the log state lives in the EVM
//! context, owned by the caller, with no global mutable state.

use revm::primitives::{Address, B256};

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

/// Chain context that collects L2→L1 logs during EVM execution.
///
/// Used as the `CHAIN` type parameter in `Context<..., CHAIN, ...>`.
/// The L1Messenger precompile calls `push_log` via `ctx.chain_mut()`.
/// The executor calls `take_logs` after each transaction.
#[derive(Debug, Default, Clone)]
pub struct ZkChainContext {
    logs: Vec<L2ToL1Log>,
    tx_number: u16,
}

impl ZkChainContext {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the current transaction number. Call before each tx execution.
    pub fn set_tx_number(&mut self, tx_number: u16) {
        self.tx_number = tx_number;
    }

    /// Push an L2→L1 log. Called by the L1Messenger precompile.
    pub fn push_log(&mut self, sender: Address, key: B256, value: B256) {
        self.logs.push(L2ToL1Log {
            l2_shard_id: 0,
            is_service: true,
            tx_number_in_block: self.tx_number,
            sender,
            key,
            value,
        });
    }

    /// Drain all logs collected during the current transaction.
    pub fn take_logs(&mut self) -> Vec<L2ToL1Log> {
        self.logs.drain(..).collect()
    }
}

/// Trait for chain contexts that can collect L2→L1 logs.
///
/// Implemented by `ZkChainContext`. The L1Messenger precompile is generic
/// over `CTX: ContextTr` and accesses this via `ctx.chain_mut()`.
/// Contexts with `Chain = ()` (no log collection) use the blanket impl
/// which silently drops logs.
pub trait L2ToL1LogStore {
    fn push_l2_to_l1_log(&mut self, sender: Address, key: B256, value: B256);
}

impl L2ToL1LogStore for ZkChainContext {
    fn push_l2_to_l1_log(&mut self, sender: Address, key: B256, value: B256) {
        self.push_log(sender, key, value);
    }
}

/// Blanket impl for `()` — silently drops logs when no chain context is configured.
impl L2ToL1LogStore for () {
    fn push_l2_to_l1_log(&mut self, _sender: Address, _key: B256, _value: B256) {}
}
