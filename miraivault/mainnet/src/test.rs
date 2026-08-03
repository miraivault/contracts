#[cfg(test)]
mod test {
    use crate::{ TimeCapsule, TimeCapsuleClient };
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        Address, Env, contractimpl, contracttype, symbol_short, contract, String, token,
    };


// Mock token contract for testing
#[contract]
pub struct MockToken;

#[contractimpl]
impl MockToken {
    pub fn initialize(env: &Env, admin: Address, decimal: u32, name: String, symbol: String) {
        env.storage()
            .instance()
            .set(&symbol_short!("admin"), &admin);
        env.storage()
            .instance()
            .set(&symbol_short!("decimal"), &decimal);
        env.storage().instance().set(&symbol_short!("name"), &name);
        env.storage()
            .instance()
            .set(&symbol_short!("symbol"), &symbol);
    }

    pub fn mint(env: &Env, to: Address, amount: i128) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&symbol_short!("admin"))
            .unwrap();
        admin.require_auth();

        let current_balance = env.storage().instance().get(&to).unwrap_or(0);
        env.storage()
            .instance()
            .set(&to, &(current_balance + amount));
    }

    pub fn balance(env: &Env, id: Address) -> i128 {
        env.storage().instance().get(&id).unwrap_or(0)
    }

    pub fn transfer(env: &Env, from: Address, to: Address, amount: i128) {
        from.require_auth();

        let from_balance = env.storage().instance().get(&from).unwrap_or(0);
        if from_balance < amount {
            panic!("insufficient balance");
        }

        let to_balance = env.storage().instance().get(&to).unwrap_or(0);
        env.storage()
            .instance()
            .set(&from, &(from_balance - amount));
        env.storage().instance().set(&to, &(to_balance + amount));
    }
}

#[contract]
pub struct MockProtocol;

#[contracttype]
#[derive(Clone)]
enum MockDataKey {
    Supplied(u64), // vault id → amount
    BaseToken, // address of the mock XLM token
}

#[contractimpl]
impl MockProtocol {
    pub fn initialize(env: Env, base_token: Address) {
        env.storage().persistent().set(&MockDataKey::BaseToken, &base_token);
    }

    pub fn supply(env: Env, vault_id: u64, caller: Address, amount: i128) {
        if amount <= 0 {
            panic!("Invalid amount");
        }
        //caller.require_auth();

        let base_token: Address = env.storage().persistent().get(&MockDataKey::BaseToken).unwrap();

        let token_client = token::Client::new(&env, &base_token);
        token_client.transfer(&caller, &env.current_contract_address(), &amount);

        let key = MockDataKey::Supplied(vault_id);
        let current: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        env.storage().persistent().set(&key, &(current + amount));
    }

    pub fn withdraw(env: Env, vault_id: u64, amount: i128, to: Address) {
        if amount <= 0 {
            panic!("Invalid amount");
        }

        let key = MockDataKey::Supplied(vault_id);
        let current: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        if amount > current { panic!("Insufficient balance in mock"); }

        env.storage().persistent().set(&key, &(current - amount));

        let base_token: Address = env.storage().persistent().get(&MockDataKey::BaseToken).unwrap();
        let token_client = token::Client::new(&env, &base_token);
        token_client.transfer(&env.current_contract_address(), &to, &amount);
    }

    pub fn get_supplied(env: Env, vault_id: u64) -> i128 {
        let key = MockDataKey::Supplied(vault_id);
        env.storage().persistent().get(&key).unwrap_or(0)
    }
}

#[test]
fn test_full_vault_lifecycle() {
    let env = Env::default();
    env.mock_all_auths_allowing_non_root_auth();

    //let vault_admin = Address::generate(&env);
    let vault_contract_id = env.register(TimeCapsule, ());
    let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

    // Mock MIRAI token
    let mirai_contract_id = env.register(MockToken, ());
    let mirai_client = MockTokenClient::new(&env, &mirai_contract_id);
    mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "Mirai"), &String::from_str(&env, "MIRAI"));
    vault_client.initialize(&mirai_contract_id, &10_000_000_i128, &5_000_000_i128, &false);
    mirai_client.mint(&vault_contract_id, &5_000_000_i128);


    // Mock vesting token (XLM-like)
    let vesting_id = env.register(MockToken, ());
    let vesting_client = MockTokenClient::new(&env, &vesting_id);
    vesting_client.initialize(&Address::generate(&env), &7u32, &String::from_str(&env, "Test XLM"), &String::from_str(&env, "XLM"));

    let sender = Address::generate(&env);
    let beneficiary = Address::generate(&env);

    vesting_client.mint(&sender, &100_000_000_000_i128);

    // Deploy mocks
    let p1_addr = env.register(MockProtocol, ());
    let p2_addr = env.register(MockProtocol, ());

    let p1_client = MockProtocolClient::new(&env, &p1_addr);
    let p2_client = MockProtocolClient::new(&env, &p2_addr);

    p1_client.initialize(&vesting_id);
    p2_client.initialize(&vesting_id);



    // Create vault
    let vault_id = vault_client.create_fund(
        &sender,
        &beneficiary,
        &vesting_id,
        &10_000_000_000_i128,
        &5u32,
        &4u32,
        &50u32,
        &Some(p1_addr.clone()),
        &40u32,
        &Some(p2_addr.clone()),
        &30u32,
    );

    // Supply
    vault_client.supply_to_p1(&vault_id, &sender, &4_000_000_000_i128);

    // Advance time
    let init_schedule = vault_client.get_vault(&vault_id);
    let seconds_per_year: u64 = 31_536_000;
    let total_duration = seconds_per_year * init_schedule.num_years as u64;
    let interval = total_duration / init_schedule.frequency as u64;
    env.ledger().set_timestamp(init_schedule.start_timestamp + interval * 5 + 1);

    // Claim
    vault_client.claim_main(&vault_id, &beneficiary);

    let this_schedule = vault_client.get_vault(&vault_id);
    assert_eq!(this_schedule.supplied_p1, 4_000_000_000_i128);
    assert_eq!(this_schedule.supplied_p2, 0);
    assert_eq!(this_schedule.claimed_packets, 5);


    // Final claim
    env.ledger().set_timestamp(100_000_000_000_000);
    vault_client.claim_main(&vault_id, &beneficiary);

    let final_schedule = vault_client.get_vault(&vault_id);
    assert_eq!(final_schedule.claimed_packets, 20);
    assert_eq!(final_schedule.supplied_p1, 0);
    assert_eq!(final_schedule.supplied_p2, 0);
}

#[test]
#[should_panic(expected = "No main packets due yet")]
fn test_nothing_to_claim_immediately() {
    let env = Env::default();
    env.mock_all_auths_allowing_non_root_auth();

    //let vault_admin = Address::generate(&env);
    let vault_contract_id = env.register(TimeCapsule, ());
    let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

    // Mock MIRAI token
    let mirai_contract_id = env.register(MockToken, ());
    let mirai_client = MockTokenClient::new(&env, &mirai_contract_id);
    mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "Mirai"), &String::from_str(&env, "MIRAI"));
    vault_client.initialize(&mirai_contract_id, &10_000_000_i128, &5_000_000_i128, &false);
    mirai_client.mint(&vault_contract_id, &5_000_000_i128);


    // Mock vesting token (XLM-like)
    let vesting_id = env.register(MockToken, ());
    let vesting_client = MockTokenClient::new(&env, &vesting_id);
    vesting_client.initialize(&Address::generate(&env), &7u32, &String::from_str(&env, "Test XLM"), &String::from_str(&env, "XLM"));

    let sender = Address::generate(&env);
    let beneficiary = Address::generate(&env);

    vesting_client.mint(&sender, &100_000_000_000_i128);

    // Deploy mocks
    let p1_addr = env.register(MockProtocol, ());
    let p2_addr = env.register(MockProtocol, ());

    // Create vault
    let vault_id = vault_client.create_fund(
        &sender,
        &beneficiary,
        &vesting_id,
        &10_000_000_000_i128,
        &5u32,
        &4u32,
        &50u32,
        &Some(p1_addr.clone()),
        &40u32,
        &Some(p2_addr.clone()),
        &30u32,
    );


    vault_client.claim_main(&vault_id, &beneficiary);
}


#[test]
fn test_cancel_immediately() {
    let env = Env::default();
    env.mock_all_auths_allowing_non_root_auth();

    let vault_contract_id = env.register(TimeCapsule, ());
    let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

    // MIRAI
    let mirai_contract_id = env.register(MockToken, ());
    let mirai_client = MockTokenClient::new(&env, &mirai_contract_id);
    mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "Mirai"), &String::from_str(&env, "MIRAI"));
    vault_client.initialize(&mirai_contract_id, &10_000_000_i128, &5_000_000_i128, &false);
    mirai_client.mint(&vault_contract_id, &5_000_000_i128);

    // Vesting token
    let vesting_id = env.register(MockToken, ());
    let vesting_client = MockTokenClient::new(&env, &vesting_id);
    vesting_client.initialize(&Address::generate(&env), &7u32, &String::from_str(&env, "Test XLM"), &String::from_str(&env, "XLM"));

    let sender = Address::generate(&env);
    let beneficiary = Address::generate(&env);

    let total_amount = 10_000_000_000_i128;
    vesting_client.mint(&sender, &total_amount);

    let initial_balance = vesting_client.balance(&sender);

    // Create vault
    let vault_id = vault_client.create_fund(
        &sender,
        &beneficiary,
        &vesting_id,
        &total_amount,
        &5u32,
        &4u32,
        &50u32,
        &None,
        &0u32,
        &None,
        &0u32,
    );

    // Immediately cancel
    vault_client.cancel_remaining(&vault_id, &sender);

    let final_balance = vesting_client.balance(&sender);
    assert_eq!(final_balance, initial_balance, "Sender should get full amount back on immediate cancel");

    let final_schedule = vault_client.get_vault(&vault_id);
    assert!(final_schedule.is_cancelled);
    assert_eq!(final_schedule.supplied_p1, 0);
    assert_eq!(final_schedule.supplied_p2, 0);
}

#[test]
fn test_cancel_after_supplying_to_protocols() {
    let env = Env::default();
    env.mock_all_auths_allowing_non_root_auth();

    let vault_contract_id = env.register(TimeCapsule, ());
    let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

    // MIRAI
    let mirai_contract_id = env.register(MockToken, ());
    let mirai_client = MockTokenClient::new(&env, &mirai_contract_id);
    mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "Mirai"), &String::from_str(&env, "MIRAI"));
    vault_client.initialize(&mirai_contract_id, &10_000_000_i128, &5_000_000_i128, &false);
    mirai_client.mint(&vault_contract_id, &5_000_000_i128);

    // Vesting token
    let vesting_id = env.register(MockToken, ());
    let vesting_client = MockTokenClient::new(&env, &vesting_id);
    vesting_client.initialize(&Address::generate(&env), &7u32, &String::from_str(&env, "Test XLM"), &String::from_str(&env, "XLM"));

    let sender = Address::generate(&env);
    let beneficiary = Address::generate(&env);

    let total_amount = 10_000_000_000_i128;
    vesting_client.mint(&sender, &total_amount);

    let initial_balance = vesting_client.balance(&sender);

    // Deploy mocks
    let p1_addr = env.register(MockProtocol, ());
    let p2_addr = env.register(MockProtocol, ());

    let p1_client = MockProtocolClient::new(&env, &p1_addr);
    let p2_client = MockProtocolClient::new(&env, &p2_addr);
    p1_client.initialize(&vesting_id);
    p2_client.initialize(&vesting_id);

    // Create vault
    let vault_id = vault_client.create_fund(
        &sender,
        &beneficiary,
        &vesting_id,
        &total_amount,
        &5u32,
        &4u32,
        &50u32,
        &Some(p1_addr.clone()),
        &40u32,
        &Some(p2_addr.clone()),
        &30u32,
    );

    // Supply 30% total (e.g. 15% each or whatever fits under ratios)
    let supply_amount = 1_500_000_000_i128; // 15% of 10B
    vault_client.supply_to_p1(&vault_id, &sender, &supply_amount);
    vault_client.supply_to_p2(&vault_id, &sender, &supply_amount);

    // Cancel
    vault_client.cancel_remaining(&vault_id, &sender);

    let final_balance = vesting_client.balance(&sender);
    assert_eq!(final_balance, initial_balance, "Sender should get full amount back after cancel");

    let final_schedule = vault_client.get_vault(&vault_id);
    assert!(final_schedule.is_cancelled);
    assert_eq!(final_schedule.supplied_p1, 0);
    assert_eq!(final_schedule.supplied_p2, 0);
    assert_eq!(final_schedule.idle_amount, 0);
}


#[test]
fn test_cancel_after_partial_claim_and_supply() {
    let env = Env::default();
    env.mock_all_auths_allowing_non_root_auth();

    let vault_contract_id = env.register(TimeCapsule, ());
    let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

    // MIRAI
    let mirai_contract_id = env.register(MockToken, ());
    let mirai_client = MockTokenClient::new(&env, &mirai_contract_id);
    mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "Mirai"), &String::from_str(&env, "MIRAI"));
    vault_client.initialize(&mirai_contract_id, &10_000_000_i128, &5_000_000_i128, &false);
    mirai_client.mint(&vault_contract_id, &5_000_000_i128);

    // Vesting token
    let vesting_id = env.register(MockToken, ());
    let vesting_client = MockTokenClient::new(&env, &vesting_id);
    vesting_client.initialize(&Address::generate(&env), &7u32, &String::from_str(&env, "Test XLM"), &String::from_str(&env, "XLM"));

    let sender = Address::generate(&env);
    let beneficiary = Address::generate(&env);

    let total_amount = 10_000_000_000_i128;
    vesting_client.mint(&sender, &total_amount);

    let initial_balance = vesting_client.balance(&sender);

    // Deploy mocks
    let p1_addr = env.register(MockProtocol, ());
    let p2_addr = env.register(MockProtocol, ());

    let p1_client = MockProtocolClient::new(&env, &p1_addr);
    let p2_client = MockProtocolClient::new(&env, &p2_addr);
    p1_client.initialize(&vesting_id);
    p2_client.initialize(&vesting_id);

    // Create vault
    let vault_id = vault_client.create_fund(
        &sender,
        &beneficiary,
        &vesting_id,
        &total_amount,
        &5u32,
        &4u32,
        &50u32,
        &Some(p1_addr.clone()),
        &40u32,
        &Some(p2_addr.clone()),
        &30u32,
    );

    // Supply 30% total (e.g. 15% each or whatever fits under ratios)
    let supply_amount = 1_500_000_000_i128; // 15% of 10B
    vault_client.supply_to_p1(&vault_id, &sender, &supply_amount);
    vault_client.supply_to_p2(&vault_id, &sender, &supply_amount);

    // Advance time
    let init_schedule = vault_client.get_vault(&vault_id);
    let seconds_per_year: u64 = 31_536_000;
    let total_duration = seconds_per_year * init_schedule.num_years as u64;
    let interval = total_duration / init_schedule.frequency as u64;
    env.ledger().set_timestamp(init_schedule.start_timestamp + interval * 5 + 1);

    // Claim
    vault_client.claim_main(&vault_id, &beneficiary);

    let this_schedule = vault_client.get_vault(&vault_id);
    assert_eq!(this_schedule.supplied_p1, 1_500_000_000_i128);
    assert_eq!(this_schedule.supplied_p2, 1_500_000_000_i128);
    assert_eq!(this_schedule.claimed_packets, 5);

    let packets_claimed = this_schedule.claimed_packets;
    let packet_amount = this_schedule.packet_amount;


    // Cancel
    vault_client.cancel_remaining(&vault_id, &sender);

    let final_balance_sender = vesting_client.balance(&sender);
    let final_balance_beneficiary = packet_amount * packets_claimed as i128;
    assert_eq!(final_balance_sender + final_balance_beneficiary, total_amount, "Sender and Beneficiary total should remain unchanged.");

    let final_schedule = vault_client.get_vault(&vault_id);
    assert!(final_schedule.is_cancelled);
    assert_eq!(final_schedule.supplied_p1, 0);
    assert_eq!(final_schedule.supplied_p2, 0);
    assert_eq!(final_schedule.idle_amount, 0);
}

#[test]
#[should_panic(expected = "Cannot allocate more than 50% to one protocol")]
fn test_supply_exceeds_50_percent_limit() {
    let env = Env::default();
    env.mock_all_auths_allowing_non_root_auth();

    let vault_contract_id = env.register(TimeCapsule, ());
    let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

    // Minimal setup (MIRAI + vesting token)
    let mirai_id = env.register(MockToken, ());
    let mirai_client = MockTokenClient::new(&env, &mirai_id);
    mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "Mirai"), &String::from_str(&env, "MIRAI"));
    vault_client.initialize(&mirai_id, &10_000_000_i128, &5_000_000_i128, &false);
    mirai_client.mint(&vault_contract_id, &5_000_000_i128);

    let vesting_id = env.register(MockToken, ());
    let vesting_client = MockTokenClient::new(&env, &vesting_id);
    vesting_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "XLM"), &String::from_str(&env, "XLM"));

    let sender = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let total = 10_000_000_000_i128;
    vesting_client.mint(&sender, &total);

    let p1_addr = env.register(MockProtocol, ());
    let p1_client = MockProtocolClient::new(&env, &p1_addr);
    p1_client.initialize(&vesting_id);

    let vault_id = vault_client.create_fund(
        &sender, &beneficiary, &vesting_id, &total,
        &5u32, &4u32, &50u32,
        &Some(p1_addr), &40u32, &None, &0u32,
    );

    // Try to supply 51% → should panic
    vault_client.supply_to_p1(&vault_id, &sender, &(total * 51 / 100));
}

#[test]
#[should_panic(expected = "Not more than 70% of the funds might be used up in third party protocols.")]
fn test_total_protocol_allocation_exceeds_70_percent() {
    let env = Env::default();

    env.mock_all_auths_allowing_non_root_auth();

    let vault_contract_id = env.register(TimeCapsule, ());
    let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

    // MIRAI
    let mirai_contract_id = env.register(MockToken, ());
    let mirai_client = MockTokenClient::new(&env, &mirai_contract_id);
    mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "Mirai"), &String::from_str(&env, "MIRAI"));
    vault_client.initialize(&mirai_contract_id, &10_000_000_i128, &5_000_000_i128, &false);
    mirai_client.mint(&vault_contract_id, &5_000_000_i128);

    // Vesting token
    let vesting_id = env.register(MockToken, ());
    let vesting_client = MockTokenClient::new(&env, &vesting_id);
    vesting_client.initialize(&Address::generate(&env), &7u32, &String::from_str(&env, "Test XLM"), &String::from_str(&env, "XLM"));

    let sender = Address::generate(&env);
    let beneficiary = Address::generate(&env);

    let total_amount = 10_000_000_000_i128;
    vesting_client.mint(&sender, &total_amount);

    let initial_balance = vesting_client.balance(&sender);

    let p1_addr = env.register(MockProtocol, ());
    let p2_addr = env.register(MockProtocol, ());
    // initialize both mocks...

    let vault_id = vault_client.create_fund(
        &sender, &beneficiary, &vesting_id, &total_amount,
        &5u32, &4u32, &50u32,
        &Some(p1_addr.clone()), &40u32,
        &Some(p2_addr.clone()), &40u32,
    );

    // Supply 40% to P1 (ok)
    vault_client.supply_to_p1(&vault_id, &sender, &(total_amount * 40 / 100));

    // Try another 40% to P2 → 80% total → should panic
    vault_client.supply_to_p2(&vault_id, &sender, &(total_amount * 40 / 100));
}


#[test]
fn test_accounting_invariant_after_supply() {
    let env = Env::default();
    env.mock_all_auths_allowing_non_root_auth();

    let vault_contract_id = env.register(TimeCapsule, ());
    let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

    // MIRAI
    let mirai_contract_id = env.register(MockToken, ());
    let mirai_client = MockTokenClient::new(&env, &mirai_contract_id);
    mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "Mirai"), &String::from_str(&env, "MIRAI"));
    vault_client.initialize(&mirai_contract_id, &10_000_000_i128, &5_000_000_i128, &false);
    mirai_client.mint(&vault_contract_id, &5_000_000_i128);

    // Vesting token
    let vesting_id = env.register(MockToken, ());
    let vesting_client = MockTokenClient::new(&env, &vesting_id);
    vesting_client.initialize(&Address::generate(&env), &7u32, &String::from_str(&env, "Test XLM"), &String::from_str(&env, "XLM"));

    let sender = Address::generate(&env);
    let beneficiary = Address::generate(&env);

    let total_amount = 10_000_000_000_i128;
    vesting_client.mint(&sender, &total_amount);

    let initial_balance = vesting_client.balance(&sender);

    let p1_addr = env.register(MockProtocol, ());
    let p2_addr = env.register(MockProtocol, ());

    let p1_client = MockProtocolClient::new(&env, &p1_addr);
    let p2_client = MockProtocolClient::new(&env, &p2_addr);
    p1_client.initialize(&vesting_id);
    p2_client.initialize(&vesting_id);

    let vault_id = vault_client.create_fund(
        &sender, &beneficiary, &vesting_id, &total_amount,
        &5u32, &4u32, &50u32,
        &Some(p1_addr.clone()), &40u32,
        &Some(p2_addr.clone()), &30u32,
    );

    let supply_p1 = total_amount * 20 / 100;
    let supply_p2 = total_amount * 20 / 100;

    vault_client.supply_to_p1(&vault_id, &sender, &supply_p1);
    vault_client.supply_to_p2(&vault_id, &sender, &supply_p2);

    let schedule = vault_client.get_vault(&vault_id);

    assert_eq!(schedule.supplied_p1, supply_p1);
    assert_eq!(schedule.supplied_p2, supply_p2);
    assert_eq!(
        schedule.supplied_p1 + schedule.supplied_p2 + schedule.idle_amount,
        total_amount,
        "Accounting invariant broken"
    );
}


#[test]
#[should_panic(expected = "Only sender can supply")]
fn test_supply_is_sender_only() {
    let env = Env::default();
    env.mock_all_auths_allowing_non_root_auth();

    let vault_contract_id = env.register(TimeCapsule, ());
    let vault_client = TimeCapsuleClient::new(&env, &vault_contract_id);

    // Minimal setup (MIRAI + vesting token)
    let mirai_id = env.register(MockToken, ());
    let mirai_client = MockTokenClient::new(&env, &mirai_id);
    mirai_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "Mirai"), &String::from_str(&env, "MIRAI"));
    vault_client.initialize(&mirai_id, &10_000_000_i128, &5_000_000_i128, &false);
    mirai_client.mint(&vault_contract_id, &5_000_000_i128);

    let vesting_id = env.register(MockToken, ());
    let vesting_client = MockTokenClient::new(&env, &vesting_id);
    vesting_client.initialize(&Address::generate(&env), &7, &String::from_str(&env, "XLM"), &String::from_str(&env, "XLM"));

    let sender = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let total = 10_000_000_000_i128;
    vesting_client.mint(&sender, &total);

    let p1_addr = env.register(MockProtocol, ());
    let p1_client = MockProtocolClient::new(&env, &p1_addr);
    p1_client.initialize(&vesting_id);

    let vault_id = vault_client.create_fund(
        &sender, &beneficiary, &vesting_id, &total,
        &5u32, &4u32, &50u32,
        &Some(p1_addr), &40u32, &None, &0u32,
    );

    // Try to supply 51% → should panic
    vault_client.supply_to_p1(&vault_id, &beneficiary, &(total * 51 / 100));
}



}

