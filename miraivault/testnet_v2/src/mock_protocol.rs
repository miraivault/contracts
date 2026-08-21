use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, token};

#[contract]
pub struct MockProtocol;

#[contracttype]
#[derive(Clone)]
enum MockDataKey {
    Supplied(u64), // vault id → amount
    BaseToken,
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
        if amount > current {
            panic!("Insufficient balance in mock protocol");
        }

        env.storage().persistent().set(&key, &(current - amount));

        let base_token: Address = env.storage().persistent().get(&MockDataKey::BaseToken).unwrap();
        let token_client = token::Client::new(&env, &base_token);
        token_client.transfer(&env.current_contract_address(), &to, &amount);
    }
}

