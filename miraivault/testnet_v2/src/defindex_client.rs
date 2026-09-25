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

    pub fn deposit(
        &self,
        amounts_desired: &Vec<i128>,
        amounts_min: &Vec<i128>,
        from: &Address,
        invest: &bool,
    ) -> i128 {
        let result: (Vec<i128>, i128, Vec<()>) = self.env.invoke_contract(
            &self.address,
            &Symbol::new(&self.env, "deposit"),
            (
                amounts_desired.clone(),
                amounts_min.clone(),
                from.clone(),
                *invest,
            )
                .into_val(&self.env),
        );
        result.1
    }

    pub fn withdraw(
        &self,
        withdraw_shares: i128,
        min_amounts_out: &Vec<i128>,
        from: &Address,
    ) -> Vec<i128> {
        let result: Vec<i128> = self.env.invoke_contract(
            &self.address,
            &Symbol::new(&self.env, "withdraw"),
            (
                withdraw_shares,
                min_amounts_out.clone(),
                from.clone(),
            )
                .into_val(&self.env),
        );
        return result;
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

    /// NEW: balance of dfTokens (shares) owned by an address
    pub fn balance(&self, owner: &Address) -> i128 {
        self.env.invoke_contract(
            &self.address,
            &Symbol::new(&self.env, "balance"),
            (owner.clone(),).into_val(&self.env),
        )
    }
}

