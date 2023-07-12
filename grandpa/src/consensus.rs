// Copyright (C) 2023 Polytope Labs.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific lang

use crate::consensus_message::ConsensusMessage;
use core::marker::PhantomData;
use ismp::{
    consensus::{
        ConsensusClient, ConsensusClientId, ConsensusStateId, StateCommitment, StateMachineClient,
    },
    error::Error,
    host::{IsmpHost, StateMachine},
    messaging::{Proof, StateCommitmentHeight},
    router::{Request, RequestResponse},
    util::hash_request,
};
use primitive_types::H256;
use primitives::{
    ConsensusState, FinalityProof, ParachainHeaderProofs, ParachainHeadersWithFinalityProof,
};
use sp_consensus_aura::AURA_ENGINE_ID;
use sp_core::H256;
use sp_runtime::DigestItem;
use std::{collections::BTreeMap, time::Duration};
use verifier::{
    verify_grandpa_finality_proof, verify_parachain_headers_with_grandpa_finality_proof,
};

pub const POLKADOT_CONSENSUS_STATE_ID: [u8; 8] = *b"polkadot";
pub const KUSAMA_CONSENSUS_STATE_ID: [u8; 8] = *b"_kusama_";

/// The `ConsensusEngineId` of ISMP digest in the parachain header.
pub const ISMP_ID: sp_runtime::ConsensusEngineId = *b"ISMP";

pub struct GrandpaConsensusClient<T>(PhantomData<(T)>);

impl<T> Default for ParachainConsensusClient<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T> ConsensusClient for GrandpaConsensusClient<T>
where
    T::BlockNumber: Into<u32>,
    T::Hash: From<H256>,
{
    fn verify_consensus(
        &self,
        host: &dyn IsmpHost,
        _consensus_state_id: ConsensusStateId,
        trusted_consensus_state: Vec<u8>,
        proof: Vec<u8>,
    ) -> Result<(Vec<u8>, BTreeMap<StateMachine, StateCommitmentHeight>), Error> {
        // decode the proof into consensus message
        let consensus_message: ConsensusMessage =
            codec::Decode::decode(&mut &proof[..]).map_err(|e| {
                Error::ImplementationSpecific(format!(
                    "Cannot decode consensus message from proof: {e:?}",
                ))
            })?;

        // decode the consensus state
        let consensus_state: ConsensusState =
            codec::Decode::decode(&mut &trusted_consensus_state[..]).map_err(|e| {
                Error::ImplementationSpecific(format!(
                    "Cannot decode consensus state from trusted consensus state bytes: {e:?}",
                ))
            })?;

        let mut intermediates = BTreeMap::new();

        // match over the message
        match consensus_message {
            ConsensusMessage::RelayChainMessage(relay_chain_message) => {
                let headers_with_finality_proof = ParachainHeadersWithFinalityProof {
                    finality_proof: relay_chain_message.finality_proof,
                    parachain_headers: relay_chain_message.parachain_headers,
                };

                let (_derived_consensus_state, parachain_headers) =
                    verify_parachain_headers_with_grandpa_finality_proof(
                        consensus_state.clone(),
                        headers_with_finality_proof,
                    )?;

                for (para_height, (header, timestamp)) in parachain_headers {
                    let mut overlay_root = H256::default();
                    for digest in header.digest().logs.iter() {
                        match digest {
                            DigestItem::Consensus(consensus_engine_id, value)
                                if *consensus_engine_id == ISMP_ID =>
                            {
                                if value.len() != 32 {
                                    Err(Error::ImplementationSpecific(
                                        "Header contains an invalid ismp root".into(),
                                    ))?
                                }

                                overlay_root = H256::from_slice(&value);
                            }
                            // don't really care about the rest
                            _ => {}
                        };
                    }

                    if timestamp == 0 {
                        Err(Error::ImplementationSpecific(
                            "Timestamp or ismp root not found".into(),
                        ))?
                    }

                    let height: u32 = (*header.number()).into();

                    let state_id = match host.host_state_machine() {
                        StateMachine::Grandpa(_) => {
                            StateMachine::Kusama(header.number().clone().into())
                        }
                        _ => Err(Error::ImplementationSpecific(
                            "Host state machine should be a parachain".into(),
                        ))?,
                    };

                    let intermediate = StateCommitmentHeight {
                        commitment: StateCommitment {
                            timestamp,
                            overlay_root: Some(overlay_root),
                            state_root: header.state_root,
                        },
                        height: height.into(),
                    };

                    intermediates.insert(state_id, intermediate);
                }

                Ok((trusted_consensus_state, intermediates))
            }

            ConsensusMessage::StandaloneChainMessage(standalone_chain_message) => {
                let (derived_consensus_state, header, _) = verify_grandpa_finality_proof(
                    consensus_state.clone(),
                    standalone_chain_message.finality_proof,
                )?;
                let (mut timestamp, mut overlay_root) = (0, H256::default());

                for digest in header.digest().logs.iter() {
                    match digest {
                        DigestItem::PreRuntime(consensus_engine_id, value)
                            if *consensus_engine_id == AURA_ENGINE_ID =>
                        {
                            let slot = Slot::decode(&mut &value[..]).map_err(|e| {
                                Error::ImplementationSpecific(format!("Cannot slot: {e:?}"))
                            })?;
                            timestamp = Duration::from_millis(*slot * SLOT_DURATION).as_secs();
                        }
                        DigestItem::Consensus(consensus_engine_id, value)
                            if *consensus_engine_id == ISMP_ID =>
                        {
                            if value.len() != 32 {
                                Err(Error::ImplementationSpecific(
                                    "Header contains an invalid ismp root".into(),
                                ))?
                            }

                            overlay_root = H256::from_slice(&value);
                        }
                        // don't really care about the rest
                        _ => {}
                    };
                }

                if timestamp == 0 {
                    Err(Error::ImplementationSpecific("Timestamp or ismp root not found".into()))?
                }

                let height: u32 = (*header.number()).into();

                let state_id = match host.host_state_machine() {
                    StateMachine::Grandpa(_) => {
                        StateMachine::Grandpa(header.number().clone().into())
                    }
                    _ => Err(Error::ImplementationSpecific(
                        "Host state machine should be grandpa".into(),
                    ))?,
                };

                let intermediate = StateCommitmentHeight {
                    commitment: StateCommitment {
                        timestamp,
                        overlay_root: Some(overlay_root),
                        state_root: header.state_root,
                    },
                    height: height.into(),
                };

                intermediates.insert(state_id, intermediate);

                Ok((trusted_consensus_state, intermediates))
            }
        }
    }

    fn verify_fraud_proof(
        &self,
        host: &dyn IsmpHost,
        trusted_consensus_state: Vec<u8>,
        proof_1: Vec<u8>,
        proof_2: Vec<u8>,
    ) -> Result<(), Error> {
        todo!()
    }

    fn state_machine(&self, id: StateMachine) -> Result<Box<dyn StateMachineClient>, Error> {
        todo!()
    }
}
