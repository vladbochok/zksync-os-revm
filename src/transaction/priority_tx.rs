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
    /// Trusted transaction hash used for the bootloader result L2→L1 log
    /// and the block header's transactions rolling hash.
    ///
    /// This value is NOT verified by the EVM — the caller is fully responsible
    /// for setting it correctly. For L1 priority txs this is the canonical
    /// priority queue hash; for upgrade txs it is the L1 upgrade tx hash.
    /// For L2 txs it is keccak256 of the EIP-2718 encoded signed bytes.
    ///
    /// The hash is included as-is in the block header commitment, so an
    /// incorrect value will produce a wrong commitment.
    pub tx_hash: B256,
}
