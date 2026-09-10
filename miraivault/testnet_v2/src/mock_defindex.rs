use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, token};

#[contract]
pub struct MockDefindex;

#[contracttype]
#[derive(Clone)]
enum MockDefindexDataKey {
    Supplied(u64),   // principal tracked
    BaseToken,
}

#[contractimpl]
impl MockDefindex {
    pub fn initialize(env: Env, base_token: Address) {
        env.storage().persistent().set(&MockDefindexDataKey::BaseToken, &base_token);
    }

    pub fn supply(env: Env, vault_id: u64, caller: Address, amount: i128) {
        if amount <= 0 {
            panic!("Invalid amount");
        }

        let base_token: Address = env.storage().persistent()
            .get(&MockDefindexDataKey::BaseToken).unwrap();

        let token_client = token::Client::new(&env, &base_token);
        token_client.transfer(&caller, &env.current_contract_address(), &amount);

        let key = MockDefindexDataKey::Supplied(vault_id);
        let current: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        env.storage().persistent().set(&key, &(current + amount));
    }

    pub fn withdraw(env: Env, vault_id: u64, amount: i128, to: Address) {
        if amount <= 0 {
            panic!("Invalid amount");
        }

        let key = MockDefindexDataKey::Supplied(vault_id);
        let current: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        if amount > current {
            panic!("Insufficient balance in mock");
        }

        // Reduce principal
        env.storage().persistent().set(&key, &(current - amount));

        // Simulate 10% yield
        let yield_amount = amount / 10;           // 10% profit
        let total_to_send = amount + yield_amount;

        let base_token: Address = env.storage().persistent()
            .get(&MockDefindexDataKey::BaseToken).unwrap();
        let token_client = token::Client::new(&env, &base_token);

        // The mock needs enough tokens to pay the yield.
        // Easiest way in tests: mint the yield to the mock first,
        // or pre-fund the mock contract with extra tokens.
        token_client.transfer(&env.current_contract_address(), &to, &total_to_send);
    }

    pub fn get_supplied(env: Env, vault_id: u64) -> i128 {
        let key = MockDefindexDataKey::Supplied(vault_id);
        env.storage().persistent().get(&key).unwrap_or(0)
    }
}
