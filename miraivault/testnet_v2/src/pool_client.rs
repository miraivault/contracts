use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Vec, token, IntoVal};


// ================== BLEND CLIENT ==================
pub const REQUEST_TYPE_SUPPLY_COLLATERAL: u32 = 2;
pub const REQUEST_TYPE_WITHDRAW_COLLATERAL: u32 = 3;

#[contracttype]
#[derive(Clone)]
pub struct Request {
    pub request_type: u32,
    pub address: Address,
    pub amount: i128,
}

// Minimal client for the Blend Pool
pub struct PoolClient<'a> {
    env: &'a Env,
    address: Address,
}

impl<'a> PoolClient<'a> {
    pub fn new(env: &'a Env, address: &Address) -> Self {
        Self {
            env,
            address: address.clone(),
        }
    }

    /// Submit one or more requests to the Blend pool
    pub fn submit(
        &self,
        from: &Address,
        spender: &Address,
        to: &Address,
        requests: &Vec<Request>,
    ) {
        let args = soroban_sdk::vec![
            self.env,
            from.clone().into_val(self.env),
            spender.clone().into_val(self.env),
            to.clone().into_val(self.env),
            requests.clone().into_val(self.env),
        ];

        self.env.invoke_contract::<()>(
            &self.address,
            &soroban_sdk::symbol_short!("submit"),
            args,
        );
    }
}
