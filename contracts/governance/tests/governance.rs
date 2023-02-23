// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

mod executor;

use dusk_bls12_381::BlsScalar;
use dusk_bls12_381_sign::{
    PublicKey as BlsPublicKey, SecretKey as BlsSecretKey,
};
use dusk_pki::{PublicKey, SecretKey};
use executor::builder::{deposit, transfer, withdraw};
use executor::tx::{self, seed};
use executor::Executor;
use governance_contract::GovernanceContract;
use microkelvin::{BackendCtor, DiskBackend, Persistence};
use rand::rngs::StdRng;
use rand::SeedableRng;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn testbackend() -> BackendCtor<DiskBackend> {
    BackendCtor::new(DiskBackend::ephemeral)
}

#[test]
fn balance_overflow() -> TestResult {
    Persistence::with_backend(&testbackend(), |_| Ok(()))?;

    let mut rng = StdRng::seed_from_u64(0xbeef);
    let alice = PublicKey::from(&SecretKey::random(&mut rng));
    let bob = PublicKey::from(&SecretKey::random(&mut rng));

    let sk_authority = BlsSecretKey::random(&mut rng);
    let authority = BlsPublicKey::from(&sk_authority);

    let mut contract = GovernanceContract::default();
    contract.authority = authority;

    let genesis_value = 100_000_000_000_000;

    let mut executor = Executor::new(2324, contract, genesis_value);
    let contract = executor.state();

    assert_eq!(contract.total_supply(), 0);
    assert_eq!(contract.balance(&alice), 0);
    assert_eq!(contract.balance(&bob), 0);

    let mint = tx::mint(&sk_authority, seed(), &alice, u64::MAX);
    let contract = executor.run(mint)?;

    assert_eq!(contract.total_supply(), u64::MAX);
    assert_eq!(contract.balance(&alice), u64::MAX);
    assert_eq!(contract.balance(&bob), 0);

    let t = transfer(200).from(bob).to(alice);

    let transfer = tx::transfer(&sk_authority, seed(), vec![t]);
    assert!(executor.run(transfer).is_err(), "transfer should overflow");

    // The balances are still the same since the transfer overflowed
    let contract = executor.state();

    assert_eq!(contract.total_supply(), u64::MAX);
    assert_eq!(contract.balance(&alice), u64::MAX);
    assert_eq!(contract.balance(&bob), 0);

    Ok(())
}

#[test]
fn same_seed() -> TestResult {
    Persistence::with_backend(&testbackend(), |_| Ok(()))?;

    let mut rng = StdRng::seed_from_u64(0xbeef);

    let sk_authority = BlsSecretKey::random(&mut rng);
    let authority = BlsPublicKey::from(&sk_authority);

    let mut contract = GovernanceContract::default();
    contract.authority = authority;

    let genesis_value = 100_000_000_000_000;

    let mut executor = Executor::new(2324, contract, genesis_value);

    let seed = BlsScalar::zero();

    let pause = tx::pause(&sk_authority, seed);
    assert!(executor.run(pause).is_ok(), "pause should succeed");

    let unpause = tx::unpause(&sk_authority, seed);
    assert!(
        executor.run(unpause).is_err(),
        "unpause should fail (same seed)"
    );

    Ok(())
}

#[test]
fn wrong_signature() -> TestResult {
    Persistence::with_backend(&testbackend(), |_| Ok(()))?;

    let mut rng = StdRng::seed_from_u64(0xbeef);

    let sk_authority = BlsSecretKey::random(&mut rng);
    let bad_sk_authority = BlsSecretKey::random(&mut rng);

    let authority = BlsPublicKey::from(&sk_authority);

    let mut contract = GovernanceContract::default();
    contract.authority = authority;

    let genesis_value = 100_000_000_000_000;

    let mut executor = Executor::new(2324, contract, genesis_value);

    let pause = tx::pause(&bad_sk_authority, seed());

    assert!(
        executor.run(pause).is_err(),
        "pause should fail (bad signature)"
    );

    let pause = tx::pause(&sk_authority, seed());

    executor.run(pause).expect("executor should pass");

    Ok(())
}

#[test]
fn mint_burn_transfer() -> TestResult {
    Persistence::with_backend(&testbackend(), |_| Ok(()))?;

    let mut rng = StdRng::seed_from_u64(0xbeef);
    let alice = PublicKey::from(&SecretKey::random(&mut rng));
    let bob = PublicKey::from(&SecretKey::random(&mut rng));

    let sk_authority = BlsSecretKey::random(&mut rng);
    let authority = BlsPublicKey::from(&sk_authority);

    let mut contract = GovernanceContract::default();
    contract.authority = authority;

    let genesis_value = 100_000_000_000_000;

    let mut executor = Executor::new(2324, contract, genesis_value);
    let contract = executor.state();

    assert_eq!(contract.total_supply(), 0);
    assert_eq!(contract.balance(&alice), 0);
    assert_eq!(contract.balance(&bob), 0);

    let mint = tx::mint(&sk_authority, seed(), &alice, 100);
    let contract = executor.run(mint)?;

    assert_eq!(contract.total_supply(), 100);
    assert_eq!(contract.balance(&alice), 100);
    assert_eq!(contract.balance(&bob), 0);

    let t_1 = transfer(200).from(alice).to(bob);

    let transfer_1 = tx::transfer(&sk_authority, seed(), vec![t_1]);
    let contract = executor.run(transfer_1)?;

    assert_eq!(contract.total_supply(), 200);
    assert_eq!(contract.balance(&alice), 0);
    assert_eq!(contract.balance(&bob), 200);

    let t_2 = transfer(50).from(bob).to(alice);

    let transfer_2 = tx::transfer(&sk_authority, seed(), vec![t_2]);
    let contract = executor.run(transfer_2)?;

    assert_eq!(contract.total_supply(), 200);
    assert_eq!(contract.balance(&alice), 50);
    assert_eq!(contract.balance(&bob), 150);

    Ok(())
}

#[test]
fn fee() -> TestResult {
    Persistence::with_backend(&testbackend(), |_| Ok(()))?;

    let mut rng = StdRng::seed_from_u64(0xbeef);
    let alice = PublicKey::from(&SecretKey::random(&mut rng));
    let bob = PublicKey::from(&SecretKey::random(&mut rng));
    let broker = PublicKey::from(&SecretKey::random(&mut rng));

    let sk_authority = BlsSecretKey::random(&mut rng);
    let authority = BlsPublicKey::from(&sk_authority);

    let mut contract = GovernanceContract::default();
    contract.authority = authority;
    contract.broker = Some(broker);

    let genesis_value = 100_000_000_000_000;

    let mut executor = Executor::new(2324, contract, genesis_value);
    let contract = executor.state();

    assert_eq!(contract.total_supply(), 0);
    assert_eq!(contract.balance(&alice), 0);
    assert_eq!(contract.balance(&bob), 0);
    assert_eq!(contract.balance(&broker), 0);

    let t_1 = transfer(200).from(alice);
    let t_2 = transfer(50).from(bob);

    let transfers = tx::fee(&sk_authority, seed(), vec![t_1, t_2]);
    let contract = executor.run(transfers)?;

    assert_eq!(contract.total_supply(), 250);
    assert_eq!(contract.balance(&alice), 0);
    assert_eq!(contract.balance(&bob), 0);
    assert_eq!(contract.balance(&broker), 250);

    let transfers = vec![
        deposit(10).to(alice),
        deposit(30).to(bob),
        withdraw(20).from(bob),
        transfer(100).from(alice).to(broker),
    ];

    let transfers = tx::transfer(&sk_authority, seed(), transfers);
    let contract = executor.run(transfers)?;

    assert_eq!(contract.total_supply(), 260);
    assert_eq!(contract.balance(&alice), 0);
    assert_eq!(contract.balance(&bob), 10);
    assert_eq!(contract.balance(&broker), 250);

    Ok(())
}