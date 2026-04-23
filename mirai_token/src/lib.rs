#![no_std]

use soroban_sdk::{
    contract, contractimpl, contractmeta, Address, Env, String, Symbol, symbol_short,
};
use soroban_token_sdk::metadata::TokenMetadata;
use soroban_token_sdk::TokenUtils;

contractmeta!(key = "title", val = "Mirai Token");
contractmeta!(key = "version", val = "0.1.0");

const INITIALIZED_KEY: Symbol = symbol_short!("INIT");
const TOTAL_SUPPLY_KEY: Symbol = symbol_short!("SUPPLY");
const ADMIN_KEY: Symbol = symbol_short!("ADMIN");

#[contract]
pub struct MiraiToken;

mod storage {
    use super::*;

    pub fn admin(env: &Env) -> Address {
        env.storage().instance().get(&ADMIN_KEY).expect("Admin not initialized")
    }

    pub fn set_admin(env: &Env, new_admin: &Address) {
        env.storage().instance().set(&ADMIN_KEY, new_admin);
    }

    pub fn set_initialized(env: &Env) {
        env.storage().instance().set(&INITIALIZED_KEY, &true);
    }

    pub fn total_supply(env: &Env) -> i128 {
        env.storage().instance().get(&TOTAL_SUPPLY_KEY).unwrap_or(0)
    }

    pub fn set_total_supply(env: &Env, supply: i128) {
        env.storage().instance().set(&TOTAL_SUPPLY_KEY, &supply);
    }

    // Standardized balance helper to ensure all functions talk to the same data
    pub fn get_balance(env: &Env, addr: Address) -> i128 {
        let key = (symbol_short!("BAL"), addr);
        env.storage().persistent().get(&key).unwrap_or(0)
    }

    pub fn set_balance(env: &Env, addr: Address, amount: i128) {
        let key = (symbol_short!("BAL"), addr);
        env.storage().persistent().set(&key, &amount);
    }

    pub fn get_allowance(env: &Env, owner: Address, spender: Address) -> i128 {
        let key = (symbol_short!("ALLOW"), owner, spender);
        // Note: In a production contract, you'd also check expiration here
        env.storage().temporary().get(&key).unwrap_or(0)
    }

    pub fn set_allowance(env: &Env, owner: Address, spender: Address, amount: i128) {
        let key = (symbol_short!("ALLOW"), owner, spender);
        env.storage().temporary().set(&key, &amount);
    }
}

#[contractimpl]
impl MiraiToken {
    pub fn initialize(
        env: Env,
        admin: Address,
        name: String,
        symbol: String,
        decimals: u32,
        initial_supply: i128,
    ) {
        if initial_supply <= 0 {
            panic!("Initial supply must be positive");
        }
        if env.storage().instance().has(&INITIALIZED_KEY) {
            panic!("Contract already initialized");
        }

        storage::set_admin(&env, &admin);

        let util = TokenUtils::new(&env);
        util.metadata().set_metadata(&TokenMetadata {
            decimal: decimals,
            name,
            symbol,
        });

        storage::set_total_supply(&env, initial_supply);
        storage::set_initialized(&env);
        storage::set_balance(&env, env.current_contract_address(), initial_supply);
    }


    pub fn balance(env: Env, id: Address) -> i128 {
        storage::get_balance(&env, id)
    }

    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();
        if amount <= 0 { panic!("Amount must be positive"); }

        let b_from = storage::get_balance(&env, from.clone());
        let b_to = storage::get_balance(&env, to.clone());

        if b_from < amount { panic!("Insufficient balance"); }

        storage::set_balance(&env, from, b_from - amount);
        storage::set_balance(&env, to, b_to + amount);
    }

    pub fn total_supply(env: Env) -> i128 {
        storage::total_supply(&env)
    }

    pub fn contract_balance(env: Env) -> i128 {
        let self_addr = env.current_contract_address();
        storage::get_balance(&env, self_addr)
    }

    pub fn transfer_admin(env: Env, new_admin: Address) {
        // Corrected check: storage::admin returns the Address, not the caller
        let current_admin = storage::admin(&env);
        current_admin.require_auth();

        storage::set_admin(&env, &new_admin);
    }

    pub fn distribute_from_treasury(env: Env, to: Address, amount: i128) {
        // 1. Only the Admin can authorize this withdrawal
        let admin = storage::admin(&env);
        admin.require_auth();

        // 2. Get the contract's own address (the Treasury)
        let treasury = env.current_contract_address();

        // 3. Perform the internal balance move
        let b_treasury = storage::get_balance(&env, treasury.clone());
        let b_to = storage::get_balance(&env, to.clone());

        if b_treasury < amount { panic!("Treasury is empty"); }

        storage::set_balance(&env, treasury, b_treasury - amount);
        storage::set_balance(&env, to, b_to + amount);
    }

    pub fn approve(env: Env, owner: Address, spender: Address, amount: i128, _expiration_ledger: u32) {
        owner.require_auth();
        if amount < 0 { panic!("amount must be positive"); }

        storage::set_allowance(&env, owner, spender, amount);
    }

    pub fn allowance(env: Env, owner: Address, spender: Address) -> i128 {
        storage::get_allowance(&env, owner, spender)
    }

    pub fn transfer_from(env: Env, spender: Address, from: Address, to: Address, amount: i128) {
        spender.require_auth();
        if amount <= 0 { panic!("amount must be positive"); }

        let current_allowance = storage::get_allowance(&env, from.clone(), spender.clone());
        if current_allowance < amount { panic!("insufficient allowance"); }

        let b_from = storage::get_balance(&env, from.clone());
        let b_to = storage::get_balance(&env, to.clone());

        if b_from < amount { panic!("insufficient balance"); }

        // Update balances
        storage::set_balance(&env, from.clone(), b_from - amount);
        storage::set_balance(&env, to, b_to + amount);

        // Update (reduce) allowance
        storage::set_allowance(&env, from, spender, current_allowance - amount);
    }


    pub fn burn(env: Env, from: Address, amount: i128) {
        from.require_auth();
        let b_from = storage::get_balance(&env, from.clone());
        if b_from < amount { panic!("insufficient balance"); }

        storage::set_balance(&env, from, b_from - amount);

        // Also reduce total supply
        let new_supply = storage::total_supply(&env) - amount;
        storage::set_total_supply(&env, new_supply);
    }

    // Metadata getters using the SDK utility (no re-entry)
    pub fn decimals(env: Env) -> u32 {
        TokenUtils::new(&env).metadata().get_metadata().decimal
    }

    pub fn name(env: Env) -> String {
        TokenUtils::new(&env).metadata().get_metadata().name
    }

    pub fn symbol(env: Env) -> String {
        TokenUtils::new(&env).metadata().get_metadata().symbol
    }
}

mod test;
