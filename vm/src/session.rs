// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use dusk_core::abi::{gen_contract_id, ContractError, Metadata};
use dusk_core::transfer::{Transaction, TRANSFER_CONTRACT};
use piecrust::{CallReceipt, Error, Session, SessionData};

use crate::VM;

/// Create a new session based on the given `VM`.
pub fn new(
    vm: &VM,
    base: [u8; 32],
    chain_id: u8,
    block_height: u64,
) -> Result<Session, Error> {
    vm.session(
        SessionData::builder()
            .base(base)
            .insert(Metadata::CHAIN_ID, chain_id)?
            .insert(Metadata::BLOCK_HEIGHT, block_height)?,
    )
}

/// Create a new genesis session based on the given [`VM`].
pub fn genesis(vm: &VM, chain_id: u8) -> Session {
    vm.session(
        SessionData::builder()
            .insert(Metadata::CHAIN_ID, chain_id)
            .expect("Inserting chain ID in metadata should succeed")
            .insert(Metadata::BLOCK_HEIGHT, 0)
            .expect("Inserting block height in metadata should succeed"),
    )
    .expect("Creating a genesis session should always succeed")
}

/// Executes a transaction, returning the receipt of the call and the gas spent.
/// The following steps are performed:
///
/// 1. Check if the transaction contains contract deployment data, and if so,
///    verifies if gas limit is enough for deployment and if the gas price is
///    sufficient for deployment. If either gas price or gas limit is not
///    sufficient for deployment, transaction is discarded.
///
/// 2. Call the "spend_and_execute" function on the transfer contract with
///    unlimited gas. If this fails, an error is returned. If an error is
///    returned the transaction should be considered unspendable/invalid, but no
///    re-execution of previous transactions is required.
///
/// 3. If the transaction contains contract deployment data, additional checks
///    are performed and if they pass, deployment is executed. The following
///    checks are performed:
///    - gas limit should be is smaller than deploy charge plus gas used for
///      spending funds
///    - transaction's bytecode's bytes are consistent with bytecode's hash
///    Deployment execution may fail for deployment-specific reasons, such as
///    for example:
///    - contract already deployed
///    - corrupted bytecode
///    If deployment execution fails, the entire gas limit is consumed and error
///    is returned.
///
/// 4. Call the "refund" function on the transfer contract with unlimited gas.
///    The amount charged depends on the gas spent by the transaction, and the
///    optional contract call in steps 2 or 3.
///
/// Note that deployment transaction will never be re-executed for reasons
/// related to deployment, as it is either discarded or it charges the
/// full gas limit. It might be re-executed only if some other transaction
/// failed to fit the block.
pub fn execute(
    session: &mut Session,
    tx: &Transaction,
    gas_per_deploy_byte: u64,
    min_deploy_points: u64,
    min_deploy_gas_price: u64,
) -> Result<CallReceipt<Result<Vec<u8>, ContractError>>, Error> {
    // Transaction will be discarded if it is a deployment transaction
    // with gas limit smaller than deploy charge.
    deploy_check(tx, gas_per_deploy_byte, min_deploy_gas_price)?;

    // Spend the inputs and execute the call. If this errors the transaction is
    // unspendable.
    let mut receipt = session.call::<_, Result<Vec<u8>, ContractError>>(
        TRANSFER_CONTRACT,
        "spend_and_execute",
        tx.strip_off_bytecode().as_ref().unwrap_or(tx),
        tx.gas_limit(),
    )?;

    // Deploy if this is a deployment transaction and spend part is successful.
    contract_deploy(
        session,
        tx,
        gas_per_deploy_byte,
        min_deploy_points,
        &mut receipt,
    );

    // Ensure all gas is consumed if there's an error in the contract call
    if receipt.data.is_err() {
        receipt.gas_spent = receipt.gas_limit;
    }

    // Refund the appropriate amount to the transaction. This call is guaranteed
    // to never error. If it does, then a programming error has occurred. As
    // such, the call to `Result::expect` is warranted.
    let refund_receipt = session
        .call::<_, ()>(
            TRANSFER_CONTRACT,
            "refund",
            &receipt.gas_spent,
            u64::MAX,
        )
        .expect("Refunding must succeed");

    receipt.events.extend(refund_receipt.events);

    Ok(receipt)
}

fn deploy_check(
    tx: &Transaction,
    gas_per_deploy_byte: u64,
    min_deploy_gas_price: u64,
) -> Result<(), Error> {
    if tx.deploy().is_some() {
        let deploy_charge =
            tx.deploy_charge(gas_per_deploy_byte, min_deploy_gas_price);

        if tx.gas_price() < min_deploy_gas_price {
            return Err(Error::Panic("gas price too low to deploy".into()));
        }
        if tx.gas_limit() < deploy_charge {
            return Err(Error::Panic("not enough gas to deploy".into()));
        }
    }

    Ok(())
}

// Contract deployment will fail and charge full gas limit in the
// following cases:
// 1) Transaction gas limit is smaller than deploy charge plus gas used for
//    spending funds.
// 2) Transaction's bytecode's bytes are not consistent with bytecode's hash.
// 3) Deployment fails for deploy-specific reasons like e.g.:
//      - contract already deployed
//      - corrupted bytecode
//      - sufficient gas to spend funds yet insufficient for deployment
fn contract_deploy(
    session: &mut Session,
    tx: &Transaction,
    gas_per_deploy_byte: u64,
    min_deploy_points: u64,
    receipt: &mut CallReceipt<Result<Vec<u8>, ContractError>>,
) {
    if let Some(deploy) = tx.deploy() {
        let gas_left = tx.gas_limit() - receipt.gas_spent;
        if receipt.data.is_ok() {
            let deploy_charge =
                tx.deploy_charge(gas_per_deploy_byte, min_deploy_points);
            let min_gas_limit = receipt.gas_spent + deploy_charge;
            if gas_left < min_gas_limit {
                receipt.data = Err(ContractError::OutOfGas);
            } else if !deploy.bytecode.verify_hash() {
                receipt.data = Err(ContractError::Panic(
                    "failed bytecode hash check".into(),
                ))
            } else {
                let result = session.deploy_raw(
                    Some(gen_contract_id(
                        &deploy.bytecode.bytes,
                        deploy.nonce,
                        &deploy.owner,
                    )),
                    deploy.bytecode.bytes.as_slice(),
                    deploy.init_args.clone(),
                    deploy.owner.clone(),
                    gas_left,
                );
                match result {
                    // Should the gas spent by the INIT method charged too?
                    Ok(_) => receipt.gas_spent += deploy_charge,
                    Err(err) => {
                        let msg = format!("failed deployment: {err:?}");
                        receipt.data = Err(ContractError::Panic(msg))
                    }
                }
            }
        }
    }
}
