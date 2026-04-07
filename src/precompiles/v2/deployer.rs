use crate::precompiles::utils::{oog_error, revert};
use crate::precompiles::v2::gas_cost::HOOK_BASE_GAS_COST;
use crate::precompiles::{
    calldata_view::CalldataView, v2::gas_cost::set_bytecode_details_extra_gas,
};
use revm::interpreter::CallInputs;
use revm::{
    Database,
    context::JournalTr,
    context_interface::ContextTr,
    interpreter::{
        Gas, InstructionResult, InterpreterResult,
        gas::{COLD_ACCOUNT_ACCESS_COST, WARM_STORAGE_READ_COST},
    },
    primitives::{Address, B256, Bytes, U256, address},
    state::Bytecode,
};

// setBytecodeDetailsEVM(address,bytes32,uint32,bytes32) - f6eca0b0
pub const SET_EVM_BYTECODE_DETAILS: &[u8] = &[0xf6, 0xec, 0xa0, 0xb0];
// forceDeployOnAddresses((bytes32,address,bool,uint256,bytes)[]) - e9f18c17
pub const FORCE_DEPLOY_ON_ADDRESSES: &[u8] = &[0xe9, 0xf1, 0x8c, 0x17];
// Contract Deployer system hook (contract) needed for all envs (force deploy)
pub const CONTRACT_DEPLOYER_ADDRESS: Address = address!("0000000000000000000000000000000000008006");

pub const L2_GENESIS_UPGRADE_ADDRESS: Address =
    address!("000000000000000000000000000000000000800f");

pub const MAX_CODE_SIZE: usize = 0x6000;

/// Global capture of bytecode hashes looked up by the deployer precompile.
/// Used by the ZiSK input builder to include these preimages in the ZiSK input.
static DEPLOYER_BYTECODE_LOOKUPS: std::sync::Mutex<Vec<B256>> = std::sync::Mutex::new(Vec::new());

/// Run the deployer precompile.
pub fn deployer_precompile_call<CTX: ContextTr>(
    ctx: &mut CTX,
    inputs: &CallInputs,
    is_delegate: bool,
) -> InterpreterResult {
    let view = CalldataView::new(ctx, &inputs.input);
    let mut calldata = view.as_slice();
    let caller = inputs.caller;
    let call_value = inputs.value.get();
    let mut gas = Gas::new(inputs.gas_limit);

    // Mirror the same behaviour as on ZKsync OS
    if is_delegate || call_value != U256::ZERO {
        return revert(gas);
    }

    // Charge base cost for calling system hook
    if !gas.record_cost(HOOK_BASE_GAS_COST) {
        return oog_error();
    }

    if calldata.len() < 4 {
        return revert(gas);
    }

    let mut selector = [0u8; 4];
    selector.copy_from_slice(&calldata[..4]);
    match selector {
        s if s == FORCE_DEPLOY_ON_ADDRESSES => {
            if inputs.is_static {
                return revert(gas);
            }

            if caller != L2_GENESIS_UPGRADE_ADDRESS {
                return revert(gas);
            }

            // Parse ForceDeployment[] from ABI-encoded calldata.
            // Layout: selector(4) + offset(32) + length(32) + entries...
            // Each entry is a dynamic struct with: bytecodeHash(32), newAddress(32),
            // callConstructor(32), value(32), input_offset(32), [input_data...]
            calldata = &calldata[4..]; // skip selector
            if calldata.len() < 64 {
                return revert(gas);
            }

            let _array_offset = U256::from_be_slice(&calldata[0..32]);
            let array_len = U256::from_be_slice(&calldata[32..64]);
            let count: usize = match array_len.try_into() {
                Ok(n) if n <= 100 => n, // sanity limit
                _ => return revert(gas),
            };

            // Copy full calldata before releasing view borrow
            let full_calldata: Vec<u8> = calldata.to_vec();
            drop(view);

            // Parse the ABI-encoded ForceDeployment[] entries.
            // full_calldata is after selector: offset(32) + length(32) + struct_offsets...
            let raw = &full_calldata[..]; // already skip selector via calldata = &calldata[4..]
            for i in 0..count {
                let struct_offset_pos = 64 + i * 32;
                if struct_offset_pos + 32 > raw.len() { break; }
                let struct_offset: usize = match U256::from_be_slice(&raw[struct_offset_pos..struct_offset_pos + 32]).try_into() {
                    Ok(o) => o,
                    Err(_) => continue,
                };
                let base = 64 + struct_offset;
                if base + 128 > raw.len() { break; }
                let bytecode_hash = B256::from_slice(&raw[base..base + 32]);
                let new_address = Address::from_slice(&raw[base + 44..base + 64]);

                // Load bytecode by hash from DB
                let bytecode = match ctx.db_mut().code_by_hash(bytecode_hash) {
                    Ok(b) if !b.is_empty() => b,
                    _ => continue,
                };

                // Deploy: set code at the target address
                ctx.journal_mut()
                    .load_account(new_address)
                    .expect("load_account");
                ctx.journal_mut().set_code(new_address, bytecode);
            }

            InterpreterResult::new(InstructionResult::Return, [].into(), gas)
        }
        s if s == SET_EVM_BYTECODE_DETAILS => {
            if inputs.is_static {
                return revert(gas);
            }

            // in future we need to handle regular(not genesis) protocol upgrades
            if caller != L2_GENESIS_UPGRADE_ADDRESS {
                return revert(gas);
            }

            // decoding according to setDeployedCodeEVM(address,bytes)
            calldata = &calldata[4..];
            if calldata.len() < 128 {
                return revert(gas);
            }

            // check that first 12 bytes in address encoding are zero
            if calldata[0..12].iter().any(|byte| *byte != 0) {
                return revert(gas);
            }
            let address = Address::from_slice(&calldata[12..32]);

            let bytecode_hash =
                B256::from_slice(calldata[32..64].try_into().expect("Always valid"));

            // Capture bytecode hash for ZiSK input builder.
            if let Ok(mut lookups) = DEPLOYER_BYTECODE_LOOKUPS.lock() {
                lookups.push(bytecode_hash);
            }

            let bytecode_length: u32 = match U256::from_be_slice(&calldata[64..96]).try_into() {
                Ok(length) => length,
                Err(_) => {
                    return revert(gas);
                }
            };

            let _observable_bytecode_hash =
                B256::from_slice(calldata[96..128].try_into().expect("Always valid"));

            // Although this can be called as a part of protocol upgrade,
            // we are checking the next invariants, just in case
            // EIP-158: reject code of length > 24576.
            if bytecode_length as usize > MAX_CODE_SIZE {
                return revert(gas);
            }

            // finished reading calldata, release borrow before mutating context
            drop(view);

            // Charge extra gas for `set_bytecode_details`
            let extra_gas = set_bytecode_details_extra_gas(bytecode_length as u64);
            if !gas.record_cost(extra_gas) {
                return oog_error();
            }

            let bytecode = ctx.db_mut().code_by_hash(bytecode_hash).expect(
                "The bytecode is expected to be pre-loaded for any deployer precompile call",
            );

            // If the bytecode isn't available (e.g., during upgrade with force-deployed contracts),
            // skip the actual deployment — the code was injected via force_preimages.
            if bytecode.is_empty() || (bytecode_length as usize) > bytecode.original_bytes().len() {
                eprintln!(
                    "DEBUG deployer: missing bytecode for hash={bytecode_hash}, len={bytecode_length}, addr={address}, available={}",
                    bytecode.original_bytes().len(),
                );
                return InterpreterResult::new(InstructionResult::Return, [].into(), gas);
            }

            let bytecode_padded = Bytecode::new_legacy(Bytes::copy_from_slice(
                &bytecode.original_bytes()[0..bytecode_length as usize],
            ));
            let account = ctx
                .journal_mut()
                .load_account(address)
                .expect("load account");
            let gas_for_access = if account.is_cold {
                COLD_ACCOUNT_ACCESS_COST
            } else {
                WARM_STORAGE_READ_COST
            };
            // Charge base cost for warm/cold read
            if !gas.record_cost(gas_for_access) {
                return oog_error();
            }

            ctx.journal_mut().touch_account(address);
            ctx.journal_mut()
                .load_account(address)
                .expect("load_account");
            ctx.journal_mut().set_code(address, bytecode_padded);
            InterpreterResult::new(InstructionResult::Return, [].into(), gas)
        }
        _ => {
            // For unhandled selectors (e.g., forceDeployOnAddresses during upgrade),
            // return success as a no-op. The actual deployments are handled externally
            // (force_preimages inject the bytecodes into the state).
            InterpreterResult::new(InstructionResult::Return, [].into(), gas)
        }
    }
}

/// Drain all captured deployer bytecode hash lookups.
pub fn drain_deployer_bytecode_lookups() -> Vec<B256> {
    match DEPLOYER_BYTECODE_LOOKUPS.lock() {
        Ok(mut v) => {
            let result = v.clone();
            v.clear();
            result
        }
        Err(_) => Vec::new(),
    }
}
