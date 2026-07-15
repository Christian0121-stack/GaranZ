//! GaranZ Escrow — a Soroban smart contract for milestone/deal escrow.
//!
//! Lifecycle:
//!   1. `create_escrow` — buyer deposits `amount` of `token` into the contract.
//!      The funds move from the buyer's wallet into the contract at this step
//!      (real on-chain transfer, requires the buyer's signature/auth).
//!   2. `approve`        — buyer marks the work as accepted. This starts a
//!      48 hour (`RELEASE_DELAY_SECS`) countdown. Requires the buyer's auth.
//!   3. `release`        — permissionless: once the countdown has elapsed,
//!      anyone may call this to move the escrowed funds to the seller. No
//!      signature from buyer or seller is required for this step — only the
//!      on-chain clock condition is checked — so the seller (or the GaranZ
//!      app on the seller's behalf) can always claim funds once eligible.
//!   4. `cancel_escrow`  — buyer may reclaim funds, but only before approval
//!      (i.e. before the 48h countdown has even started). Prevents funds from
//!      being deposited by mistake or abandoned mid-deal.
//!
//! There is intentionally no arbiter/dispute path in this version — the only
//! two ways out of `Funded` are `approve` (by buyer) or `cancel_escrow` (by
//! buyer), and the only way out of `Approved` is `release` after the delay.

#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, token, Address, Env,
    String,
};

/// Countdown between buyer approval and the funds becoming claimable by the
/// seller. 48 hours, expressed in seconds (Soroban ledger timestamps are unix
/// seconds).
pub const RELEASE_DELAY_SECS: u64 = 48 * 60 * 60;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EscrowStatus {
    Funded,
    Approved,
    Released,
    Refunded,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Escrow {
    pub buyer: Address,
    pub seller: Address,
    pub token: Address,
    pub amount: i128,
    pub status: EscrowStatus,
    /// Ledger timestamp (unix seconds) at which `approve` was called. 0 until approved.
    pub approved_at: u64,
}

#[contracttype]
enum DataKey {
    Escrow(String),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    /// An escrow with this id already exists.
    AlreadyExists = 1,
    /// No escrow found for this id.
    NotFound = 2,
    /// The escrow is not in the required state for this operation.
    InvalidStatus = 3,
    /// `amount` must be a positive value.
    InvalidAmount = 4,
    /// The 48h release delay has not elapsed yet.
    TooEarly = 5,
}

#[contract]
pub struct EscrowContract;

#[contractimpl]
impl EscrowContract {
    /// Buyer deposits `amount` of `token` into escrow for `seller`, tracked
    /// under `escrow_id`. Transfers the funds from the buyer's wallet into
    /// this contract. Requires the buyer's authorization.
    pub fn create_escrow(
        env: Env,
        escrow_id: String,
        buyer: Address,
        seller: Address,
        token: Address,
        amount: i128,
    ) -> Escrow {
        buyer.require_auth();

        if amount <= 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }

        let key = DataKey::Escrow(escrow_id);
        if env.storage().persistent().has(&key) {
            panic_with_error!(&env, Error::AlreadyExists);
        }

        // Real on-chain transfer: buyer -> this contract.
        let token_client = token::Client::new(&env, &token);
        token_client.transfer(&buyer, &env.current_contract_address(), &amount);

        let escrow = Escrow {
            buyer,
            seller,
            token,
            amount,
            status: EscrowStatus::Funded,
            approved_at: 0,
        };
        env.storage().persistent().set(&key, &escrow);
        escrow
    }

    /// Buyer approves the deal. Starts the 48h countdown to auto-release.
    /// Requires the buyer's authorization.
    pub fn approve(env: Env, escrow_id: String) -> Escrow {
        let key = DataKey::Escrow(escrow_id);
        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic_with_error!(&env, Error::NotFound));

        escrow.buyer.require_auth();

        if escrow.status != EscrowStatus::Funded {
            panic_with_error!(&env, Error::InvalidStatus);
        }

        escrow.status = EscrowStatus::Approved;
        escrow.approved_at = env.ledger().timestamp();
        env.storage().persistent().set(&key, &escrow);
        escrow
    }

    /// Permissionless: releases escrowed funds to the seller once the
    /// buyer has approved AND at least `RELEASE_DELAY_SECS` have passed
    /// since approval. No signature is required from either party for this
    /// call — only the on-chain time condition is enforced — so the seller
    /// (or anyone) can always claim once eligible.
    pub fn release(env: Env, escrow_id: String) -> Escrow {
        let key = DataKey::Escrow(escrow_id);
        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic_with_error!(&env, Error::NotFound));

        if escrow.status != EscrowStatus::Approved {
            panic_with_error!(&env, Error::InvalidStatus);
        }

        let now = env.ledger().timestamp();
        if now < escrow.approved_at + RELEASE_DELAY_SECS {
            panic_with_error!(&env, Error::TooEarly);
        }

        let token_client = token::Client::new(&env, &escrow.token);
        token_client.transfer(&env.current_contract_address(), &escrow.seller, &escrow.amount);

        escrow.status = EscrowStatus::Released;
        env.storage().persistent().set(&key, &escrow);
        escrow
    }

    /// Buyer reclaims funds, only allowed before approval (i.e. before the
    /// countdown has started). Requires the buyer's authorization.
    pub fn cancel_escrow(env: Env, escrow_id: String) -> Escrow {
        let key = DataKey::Escrow(escrow_id);
        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic_with_error!(&env, Error::NotFound));

        escrow.buyer.require_auth();

        if escrow.status != EscrowStatus::Funded {
            panic_with_error!(&env, Error::InvalidStatus);
        }

        let token_client = token::Client::new(&env, &escrow.token);
        token_client.transfer(&env.current_contract_address(), &escrow.buyer, &escrow.amount);

        escrow.status = EscrowStatus::Refunded;
        env.storage().persistent().set(&key, &escrow);
        escrow
    }

    /// Read-only lookup of an escrow's current state.
    pub fn get_escrow(env: Env, escrow_id: String) -> Escrow {
        env.storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .unwrap_or_else(|| panic_with_error!(&env, Error::NotFound))
    }

    /// Seconds remaining before `release` becomes callable. 0 if already
    /// eligible or not yet approved/found in a releasable state.
    pub fn seconds_until_release(env: Env, escrow_id: String) -> u64 {
        let escrow: Escrow = env
            .storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .unwrap_or_else(|| panic_with_error!(&env, Error::NotFound));

        if escrow.status != EscrowStatus::Approved {
            return 0;
        }
        let now = env.ledger().timestamp();
        let unlock_at = escrow.approved_at + RELEASE_DELAY_SECS;
        if now >= unlock_at {
            0
        } else {
            unlock_at - now
        }
    }
}

#[cfg(test)]
mod test;
