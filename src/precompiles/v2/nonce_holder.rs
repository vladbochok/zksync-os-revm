use crate::precompiles::calldata_view::CalldataView;
use crate::precompiles::utils::revert;
use revm::interpreter::CallInputs;
use revm::{
    context::JournalTr,
    context_interface::ContextTr,
    interpreter::{Gas, InstructionResult, InterpreterResult},
    primitives::Address,
};

pub const NONCE_HOLDER_ADDRESS: Address =
    revm::primitives::address!("0000000000000000000000000000000000008003");

pub fn nonce_holder_precompile_call<CTX: ContextTr>(
    ctx: &mut CTX,
    inputs: &CallInputs,
    _is_delegate: bool,
) -> InterpreterResult {
    let view = CalldataView::new(ctx, &inputs.input);
    let calldata = view.as_slice();
    let gas = Gas::new(inputs.gas_limit);
    if calldata.len() < 4 { return revert(gas); }
    let selector = [calldata[0], calldata[1], calldata[2], calldata[3]];
    match selector {
        // getDeploymentNonce(address)
        [0xfb, 0x1a, 0x9a, 0x57] => {
            if calldata.len() < 36 { return revert(gas); }
            let address = Address::from_slice(&calldata[16..36]);
            drop(view);
            let nonce = ctx.journal_mut().load_account(address)
                .map(|a| a.info.nonce).unwrap_or(0);
            let mut out = [0u8; 32];
            out[24..32].copy_from_slice(&nonce.to_be_bytes());
            InterpreterResult::new(InstructionResult::Return, out.to_vec().into(), gas)
        }
        // incrementDeploymentNonce(address)
        [0x30, 0x63, 0x95, 0xc6] => {
            // Return new nonce = 1. The deployer precompile handles actual deployment.
            drop(view);
            let mut out = [0u8; 32];
            out[31] = 1;
            InterpreterResult::new(InstructionResult::Return, out.to_vec().into(), gas)
        }
        _ => { drop(view); InterpreterResult::new(InstructionResult::Return, [].into(), gas) }
    }
}
