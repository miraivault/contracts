#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Env, String};
use crate::{ MiraiToken, MiraiTokenClient };

#[test]
fn test_initialize_mints_full_supply_to_treasury() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);

    // Register the contract
    let contract_id = env.register(MiraiToken, ());

    // Use the automatically generated client (this is the correct way)
    let client = MiraiTokenClient::new(&env, &contract_id);

    let name = String::from_str(&env, "Mirai Token");
    let symbol = String::from_str(&env, "MIRAI");
    let decimals: u32 = 7;
    let initial_supply: i128 = 1_000_000_000_000_000_i128; // 1 billion tokens

    // Initialize (this also mints the full supply to the treasury)
    client.initialize(&admin, &name, &symbol, &decimals, &initial_supply);

    // Assertions
    assert_eq!(client.total_supply(), initial_supply);
    assert_eq!(client.contract_balance(), initial_supply, "Treasury should hold full supply");

    assert_eq!(client.name(), name);
    assert_eq!(client.symbol(), symbol);
    assert_eq!(client.decimals(), decimals);
}

#[test]
fn test_transfer_from_treasury() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    let contract_id = env.register(crate::MiraiToken, ());
    let client = crate::MiraiTokenClient::new(&env, &contract_id);

    let supply = 100_000_i128;
    client.initialize(&admin, &String::from_str(&env, "Test"), &String::from_str(&env, "TEST"), &7, &supply);

    let token_contract = contract_id.clone();

    // Transfer from treasury to user
    client.transfer(&token_contract, &user, &25_000);

    assert_eq!(client.balance(&user), 25_000);
    assert_eq!(client.contract_balance(), supply - 25_000);
}

#[test]
fn test_approve_and_transfer_from() {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let spender = Address::generate(&env);
    let recipient = Address::generate(&env);

    let contract_id = env.register(crate::MiraiToken, ());
    let client = crate::MiraiTokenClient::new(&env, &contract_id);

    let supply = 100_000_i128;
    client.initialize(&owner, &String::from_str(&env, "Mirai"), &String::from_str(&env, "MIRAI"), &7, &supply);

    // Distribute
    let distributed_amount = 70_000_i128;
    client.distribute_from_treasury(&owner, &distributed_amount);

    // Approve
    let approve_amount = 30_000_i128;
    let expiration = 10_000_000u32;
    client.approve(&owner, &spender, &approve_amount, &expiration);

    assert_eq!(client.allowance(&owner, &spender), approve_amount);

    // Spender transfers from owner
    client.transfer_from(&spender, &owner, &recipient, &12_000);

    assert_eq!(client.balance(&recipient), 12_000);
    assert_eq!(client.allowance(&owner, &spender), 18_000);
}

#[test]
fn test_burn_from_user() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    let contract_id = env.register(crate::MiraiToken, ());
    let client = crate::MiraiTokenClient::new(&env, &contract_id);

    let supply = 50_000_i128;
    client.initialize(&admin, &String::from_str(&env, "Test"), &String::from_str(&env, "TEST"), &7, &supply);

    let token_contract = contract_id.clone();

    // Give tokens to user from treasury
    client.transfer(&token_contract, &user, &10_000);

    // Burn
    client.burn(&user, &4_000);

    assert_eq!(client.balance(&user), 6_000);
}

#[test]
#[should_panic(expected = "Contract already initialized")]
fn test_cannot_initialize_twice() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);

    let contract_id = env.register(crate::MiraiToken, ());
    let client = crate::MiraiTokenClient::new(&env, &contract_id);

    client.initialize(&admin, &String::from_str(&env, "Test"), &String::from_str(&env, "TEST"), &7, &10000);
    client.initialize(&admin, &String::from_str(&env, "Test2"), &String::from_str(&env, "T2"), &7, &10000); // should panic
}

#[test]
fn test_contract_balance_decreases_on_transfer() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);

    let contract_id = env.register(crate::MiraiToken, ());
    let client = crate::MiraiTokenClient::new(&env, &contract_id);

    let supply = 100_000_i128;
    client.initialize(&admin, &String::from_str(&env, "Test"), &String::from_str(&env, "TEST"), &7, &supply);

    let token_contract = contract_id.clone();
    let user = Address::generate(&env);

    client.transfer(&token_contract, &user, &35_000);

    assert_eq!(client.contract_balance(), 65_000);
    assert_eq!(client.total_supply(), supply);
}
