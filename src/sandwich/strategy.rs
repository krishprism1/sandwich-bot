use bounded_vec_deque::BoundedVecDeque;
use ethers::signers::{LocalWallet, Signer};
use ethers::{
    providers::{Middleware, Provider, Ws},
    types::{BlockNumber, H160, H256, U256, U64},
};
use log::{info, warn};
use std::{collections::HashMap, str::FromStr, sync::Arc};
use tokio::sync::broadcast::Sender;

// we'll update this part later, for now just import the necessary components
use crate::common::constants::{Env, WETH};
use crate::common::streams::{Event, NewBlock};
use crate::common::utils::{calculate_next_block_base_fee, to_h160};
use crate::sandwich::simulation::debug_trace_call;

pub async fn run_sandwich_strategy(provider: Arc<Provider<Ws>>, event_sender: Sender<Event>) {
    let block = provider
        .get_block(BlockNumber::Latest)
        .await
        .unwrap()
        .unwrap();
    let mut new_block = NewBlock {
        block_number: block.number.unwrap(),
        base_fee: block.base_fee_per_gas.unwrap(),
        next_base_fee: calculate_next_block_base_fee(
            block.gas_used,
            block.gas_limit,
            block.base_fee_per_gas.unwrap(),
        ),
    };

    let mut event_receiver = event_sender.subscribe();

    loop {
        match event_receiver.recv().await {
            Ok(event) => match event {
                Event::Block(block) => {
                    new_block = block;
                    info!("[Block #{:?}]", new_block.block_number);
                }
                Event::PendingTx(mut pending_tx) => {
                    let frame = debug_trace_call(&provider, &new_block, &pending_tx).await;
                    match frame {
                        Ok(frame) => info!("{:?}", frame),
                        Err(e) => info!("{e:?}"),
                    }
                }
            },
            _ => {}
        }
    }
}