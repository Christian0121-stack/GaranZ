#![cfg(test)]

use super::*;
use soroban_sdk::testutils::{Address as _, Ledger, LedgerInfo};
use soroban_sdk::{Env, String};

fn setup(env: &Env) -> (Address, Address, Address, token::Client<'_>, token::StellarAssetClient<'_>) {
    let admin = Address::generate(env);
    let buyer = Address::generate(env);
    let seller = Address::generate(env);
    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    let token = token::Client::new(env, &sac.address());
    let token_admin = token::StellarAssetClient::new(env, &sac.address());
    (buyer, seller, sac.address(), token, token_admin)
}

fn advance_time(env: &Env, by_secs: u64) {
    let now = env.ledger().timestamp();
    env.ledger().set(LedgerInfo {
        timestamp: now + by_secs,
        protocol_version: env.ledger().protocol_version(),
        sequence_number: env.ledger().sequence(),
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 16 * 60 * 60 * 24,
        min_persistent_entry_ttl: 30 * 60 * 60 * 24,
        max_entry_ttl: 365 * 60 * 60 * 24,
    });
}

#[test]
fn full_happy_path_release_after_delay() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000);
    let (buyer, seller, token_addr, token, token_admin) = setup(&env);

    token_admin.mint(&buyer, &1_000_0000000i128);

    let contract_id = env.register(EscrowContract, ());
    let client = EscrowContractClient::new(&env, &contract_id);

    let escrow_id = String::from_str(&env, "deal1_m0");
    let amount = 500_0000000i128; // 500 XLM in stroops

    let escrow = client.create_escrow(&escrow_id, &buyer, &seller, &token_addr, &amount);
    assert_eq!(escrow.status, EscrowStatus::Funded);
    assert_eq!(token.balance(&contract_id), amount);
    assert_eq!(token.balance(&buyer), 500_0000000i128);

    // Cannot release before approval.
    let seconds_left = client.seconds_until_release(&escrow_id);
    assert_eq!(seconds_left, 0); // not approved yet => 0 by convention

    let escrow = client.approve(&escrow_id);
    assert_eq!(escrow.status, EscrowStatus::Approved);
    assert!(escrow.approved_at > 0);

    // Too early — releasing right after approval must fail.
    let result = client.try_release(&escrow_id);
    assert!(result.is_err());

    let remaining = client.seconds_until_release(&escrow_id);
    assert_eq!(remaining, RELEASE_DELAY_SECS);

    // Fast-forward 47h59m59s — still too early.
    advance_time(&env, RELEASE_DELAY_SECS - 1);
    let result = client.try_release(&escrow_id);
    assert!(result.is_err());

    // Fast-forward the final second — now eligible.
    advance_time(&env, 1);
    let escrow = client.release(&escrow_id);
    assert_eq!(escrow.status, EscrowStatus::Released);
    assert_eq!(token.balance(&seller), amount);
    assert_eq!(token.balance(&contract_id), 0);

    // Releasing twice must fail (already released).
    let result = client.try_release(&escrow_id);
    assert!(result.is_err());
}

#[test]
fn cancel_before_approval_refunds_buyer() {
    let env = Env::default();
    env.mock_all_auths();
    let (buyer, seller, token_addr, token, token_admin) = setup(&env);
    token_admin.mint(&buyer, &1_000_0000000i128);

    let contract_id = env.register(EscrowContract, ());
    let client = EscrowContractClient::new(&env, &contract_id);

    let escrow_id = String::from_str(&env, "deal42");
    let amount = 100_0000000i128;
    client.create_escrow(&escrow_id, &buyer, &seller, &token_addr, &amount);
    assert_eq!(token.balance(&buyer), 900_0000000i128);

    let escrow = client.cancel_escrow(&escrow_id);
    assert_eq!(escrow.status, EscrowStatus::Refunded);
    assert_eq!(token.balance(&buyer), 1_000_0000000i128);
    assert_eq!(token.balance(&contract_id), 0);

    // Cannot cancel again once refunded.
    let result = client.try_cancel_escrow(&escrow_id);
    assert!(result.is_err());
}

#[test]
fn cannot_cancel_after_approval() {
    let env = Env::default();
    env.mock_all_auths();
    let (buyer, seller, token_addr, _token, token_admin) = setup(&env);
    token_admin.mint(&buyer, &1_000_0000000i128);

    let contract_id = env.register(EscrowContract, ());
    let client = EscrowContractClient::new(&env, &contract_id);

    let escrow_id = String::from_str(&env, "deal7");
    client.create_escrow(&escrow_id, &buyer, &seller, &token_addr, &100_0000000i128);
    client.approve(&escrow_id);

    let result = client.try_cancel_escrow(&escrow_id);
    assert!(result.is_err());
}

#[test]
fn duplicate_escrow_id_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (buyer, seller, token_addr, _token, token_admin) = setup(&env);
    token_admin.mint(&buyer, &1_000_0000000i128);

    let contract_id = env.register(EscrowContract, ());
    let client = EscrowContractClient::new(&env, &contract_id);

    let escrow_id = String::from_str(&env, "deal99");
    client.create_escrow(&escrow_id, &buyer, &seller, &token_addr, &10_0000000i128);
    let result = client.try_create_escrow(&escrow_id, &buyer, &seller, &token_addr, &10_0000000i128);
    assert!(result.is_err());
}

#[test]
fn zero_or_negative_amount_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (buyer, seller, token_addr, _token, _token_admin) = setup(&env);

    let contract_id = env.register(EscrowContract, ());
    let client = EscrowContractClient::new(&env, &contract_id);

    let escrow_id = String::from_str(&env, "deal1");
    let result = client.try_create_escrow(&escrow_id, &buyer, &seller, &token_addr, &0i128);
    assert!(result.is_err());
}

#[test]
fn get_escrow_not_found_panics() {
    let env = Env::default();
    let contract_id = env.register(EscrowContract, ());
    let client = EscrowContractClient::new(&env, &contract_id);
    let escrow_id = String::from_str(&env, "nonexistent");
    let result = client.try_get_escrow(&escrow_id);
    assert!(result.is_err());
}
