//! Contains Deposit transaction parts.
use revm::primitives::{Address, B256, U256};

/// Upgrade transaction type.
pub const UPGRADE_TRANSACTION_TYPE: u8 = 0x7E;

/// Priority transaction type.
pub const L1_PRIORITY_TRANSACTION_TYPE: u8 = 0x7f;

/// Deposit transaction parts.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct L1ToL2TransactionParts {
    pub mint: Option<U256>,
    pub refund_recipient: Option<Address>,
    /// L1 transaction hash — used to emit the bootloader result L2→L1 log.
    pub l1_tx_hash: Option<B256>,
}
