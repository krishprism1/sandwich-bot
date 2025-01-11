use anyhow::Result;
use eth_encode_packed::ethabi::ethereum_types::{H160 as eH160, U256 as eU256};
use eth_encode_packed::{SolidityDataType, TakeLastXBytes};
use ethers::abi::ParamType;
use ethers::prelude::*;
use ethers::providers::{Provider, Ws};
use ethers::types::{transaction::eip2930::AccessList, Bytes, H160, H256, I256, U256, U64};
use log::info;
use revm::primitives::{Bytecode, U256 as rU256};
use std::{collections::HashMap, default::Default, str::FromStr, sync::Arc};

use crate::common::constants::{WETH, WETH_BALANCE_SLOT};
use crate::common::streams::{NewBlock, NewPendingTx};
use crate::common::utils::{create_new_wallet, is_weth, to_h160};

#[derive(Debug, Clone, Default)]
pub struct PendingTxInfo {
    pub pending_tx: NewPendingTx,
    pub touched_pairs: Vec<SwapInfo>,
}

#[derive(Debug, Clone)]
pub enum SwapDirection {
    Buy,
    Sell,
}

#[derive(Debug, Clone)]
pub struct SwapInfo {
    pub tx_hash: H256,
    pub target_pair: H160,
    pub main_currency: H160,
    pub target_token: H160,
    pub version: u8,
    pub token0_is_main: bool,
    pub direction: SwapDirection,
}

pub static V2_SWAP_EVENT_ID: &str = "0xd78ad95f";

pub async fn debug_trace_call(
    provider: &Arc<Provider<Ws>>,
    new_block: &NewBlock,
    pending_tx: &NewPendingTx,
) -> Result<Option<CallFrame>> {
    let mut opts = GethDebugTracingCallOptions::default();
    let mut call_config = CallConfig::default();
    call_config.with_log = Some(true); // 👈 make sure we are getting logs

    opts.tracing_options.tracer = Some(GethDebugTracerType::BuiltInTracer(
        GethDebugBuiltInTracerType::CallTracer,
    ));
    opts.tracing_options.tracer_config = Some(GethDebugTracerConfig::BuiltInTracer(
        GethDebugBuiltInTracerConfig::CallTracer(call_config),
    ));

    let block_number = new_block.block_number;
    let mut tx = pending_tx.tx.clone();
    let nonce = provider
        .get_transaction_count(tx.from, Some(block_number.into()))
        .await
        .unwrap_or_default();
    tx.nonce = nonce;

    let trace = provider
        .debug_trace_call(&tx, Some(block_number.into()), opts)
        .await;

    match trace {
        Ok(trace) => match trace {
            GethTrace::Known(call_tracer) => match call_tracer {
                GethTraceFrame::CallTracer(frame) => Ok(Some(frame)),
                _ => Ok(None),
            },
            _ => Ok(None),
        },
        _ => Ok(None),
    }
}

pub fn extract_logs(call_frame: &CallFrame, logs: &mut Vec<CallLogFrame>) {
    if let Some(ref logs_vec) = call_frame.logs {
        logs.extend(logs_vec.iter().cloned());
    }

    if let Some(ref calls_vec) = call_frame.calls {
        for call in calls_vec {
            extract_logs(call, logs);
        }
    }
}