use crate::precompiles::utils::revert;
use revm::interpreter::CallInputs;
use revm::{
    context_interface::ContextTr,
    interpreter::{Gas, InstructionResult, InterpreterResult},
    primitives::Address,
};

pub const KNOWN_CODES_STORAGE_ADDRESS: Address =
    revm::primitives::address!("0000000000000000000000000000000000008004");

pub fn known_codes_precompile_call<CTX: ContextTr>(
    _ctx: &mut CTX,
    inputs: &CallInputs,
    _is_delegate: bool,
) -> InterpreterResult {
    let gas = Gas::new(inputs.gas_limit);
    if inputs.input.len() < 4 { return revert(gas); }
    // getMarker(bytes32) → return 1 (all bytecodes known)
    let mut out = [0u8; 32];
    out[31] = 1;
    InterpreterResult::new(InstructionResult::Return, out.to_vec().into(), gas)
}
