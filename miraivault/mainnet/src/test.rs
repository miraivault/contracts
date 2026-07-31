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
    env.mock_all_auths();

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
    env.ledger().set_timestamp(100_000_000_000);

    // Claim
    vault_client.claim_main(&vault_id, &beneficiary);

    let this_schedule = vault_client.get_vault(&vault_id);
    assert_eq!(this_schedule.supplied_p1, 4_000_000_000_i128);
    assert_eq!(this_schedule.supplied_p2, 0);


    // Final claim
    env.ledger().set_timestamp(100_000_000_000_000);
    vault_client.claim_main(&vault_id, &beneficiary);

    let final_schedule = vault_client.get_vault(&vault_id);
    assert_eq!(final_schedule.supplied_p1, 0);
    assert_eq!(final_schedule.supplied_p2, 0);
}

#[test]
#[should_panic(expected = "No main packets due yet")]
fn test_nothing_to_claim_immediately() {
    let env = Env::default();
    env.mock_all_auths();

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

}
