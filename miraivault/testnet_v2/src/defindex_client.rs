use soroban_sdk::{Address, Env, Vec, IntoVal, Symbol};

pub struct DefindexClient {
    env: Env,
    address: Address,
}

impl DefindexClient {
    pub fn new(env: &Env, address: &Address) -> Self {
        Self {
            env: env.clone(),
            address: address.clone(),
        }
    }

    pub fn deposit(&self, from: &Address, amount: i128, min_shares: i128) {
        self.env.invoke_contract::<()>(
            &self.address,
            &Symbol::new(&self.env, "deposit"),
            (from.clone(), amount, min_shares).into_val(&self.env),
        );
    }

    pub fn withdraw(
        &self,
        shares: i128,
        min_amounts_out: &Vec<i128>,
        from: &Address,
    ) {
        self.env.invoke_contract::<()>(
            &self.address,
            &Symbol::new(&self.env, "withdraw"),
            (shares, min_amounts_out.clone(), from.clone()).into_val(&self.env),
        );
    }

    pub fn total_supply(&self) -> i128 {
        self.env.invoke_contract(
            &self.address,
            &Symbol::new(&self.env, "total_supply"),
            ().into_val(&self.env),
        )
    }

    pub fn fetch_total_managed_funds(&self) -> Vec<i128> {
        self.env.invoke_contract(
            &self.address,
            &Symbol::new(&self.env, "fetch_total_managed_funds"),
            ().into_val(&self.env),
        )
    }
}

