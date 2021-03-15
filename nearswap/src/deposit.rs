use std::collections::HashMap;
use std::convert::TryInto;

use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
use near_sdk::borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::{env, near_bindgen, AccountId, Balance, PromiseOrValue, StorageUsage};

//use crate::errors::*;
use crate::ft_token::*;
use crate::*;

// TODO: move to other place
const STORAGE_PRICE_PER_BYTE: Balance = env::STORAGE_PRICE_PER_BYTE;

/**********************
   DEPOSIT AND STORAGE
       MANAGEMENT
***********************/

// token deposits are done through NEP-141 ft_transfer_call to the NEARswap contract.
#[near_bindgen]
impl FungibleTokenReceiver for NearSwap {
    /**
    Callback on receiving tokens by this contract.
    Returns zero.
    Panics when account is not registered. */
    #[allow(unused_variables)]
    fn ft_on_transfer(
        &mut self,
        sender_id: ValidAccountId,
        amount: U128,
        msg: String,
    ) -> PromiseOrValue<U128> {
        let token = env::predecessor_account_id();
        let sender_id = AccountId::from(sender_id);

        let mut d = self.get_deposit(&sender_id);
        d.add(&token, amount.into());
        self.deposits.insert(&sender_id, &d);
        env_log!("Deposit, {} {}", amount.0, token);

        return PromiseOrValue::Value(U128(0));
    }
}

#[near_bindgen]
impl NearSwap {
    /**
    Deposits attached NEAR.
    Panics if the sender account is not registered. */
    #[payable]
    pub fn deposit_near(&mut self) {
        let sender = env::predecessor_account_id();
        let mut d = self.get_deposit(&sender);
        let amount = env::attached_deposit();
        d.near += amount;
        self.deposits.insert(&sender, &d);
        env_log!("Deposit, {} yNEAR", amount);
    }

    pub fn withdraw_near_deposit(
        &mut self,
        amount: U128,
        recipient: Option<ValidAccountId>,
    ) -> Promise {
        let sender = env::predecessor_account_id();
        let recipient = if let Some(a) = recipient {
            AccountId::from(a)
        } else {
            sender.clone()
        };
        env_log!("Deposit withdraw, {} yNEAR", amount.0);
        let amount = u128::from(amount);
        let mut d = self.get_deposit(&sender);
        d.assert_near(amount);
        d.near -= amount;
        self.deposits.insert(&sender, &d);
        Promise::new(recipient).transfer(amount)
    }

    pub fn withdraw_token_deposit(
        &mut self,
        token: ValidAccountId,
        amount: U128,
        recipient: Option<ValidAccountId>,
        is_contract: bool,
        tx_call_msg: String,
    ) {
        let sender = env::predecessor_account_id();
        let recipient = if let Some(a) = recipient {
            AccountId::from(a)
        } else {
            sender.clone()
        };
        let token_acc = AccountId::from(token.clone());
        env_log!("Deposit withdraw, {} {}", amount.0, token_acc);
        let mut d = self.get_deposit(&sender);
        let amount = u128::from(amount);
        d.remove(&token_acc, amount);
        self.deposits.insert(&sender, &d);

        if is_contract {
            ext_fungible_token::ft_transfer(
                recipient.try_into().unwrap(),
                amount.into(),
                Some("NEARswap withdraw".to_string()),
                token.as_ref(),
                1, // required 1yNEAR for transfers
                GAS_FOR_FT_TRANSFER,
            );
        } else {
            ext_fungible_token::ft_transfer_call(
                recipient.try_into().unwrap(),
                amount.into(),
                Some("NEARswap withdraw".to_string()),
                tx_call_msg,
                token.as_ref(),
                1, // required 1yNEAR for transfers
                GAS_FOR_FT_TRANSFER,
            );
        }
    }

    #[inline]
    fn get_deposit(&self, from: &AccountId) -> AccountDeposit {
        self.deposits.get(from).expect(ERR20_ACC_NOT_REGISTERED)
    }
}

/// Account deposits information and storage cost.
#[cfg(not(test))]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct AccountDeposit {
    /// Native amount sent to the exchange.
    /// Used for storage now, but in future can be used for trading as well.
    /// MUST be always bigger than `storage_used * STORAGE_PRICE_PER_BYTE`.
    pub near: Balance,
    /// Amount of storage bytes used by the account,
    pub storage_used: StorageUsage,
    /// Deposited token balances.
    pub tokens: HashMap<AccountId, Balance>,
}

/// Account deposits information and storage cost.
#[cfg(test)]
#[derive(BorshSerialize, BorshDeserialize, Default, Clone)]
pub struct AccountDeposit {
    pub near: Balance,
    pub storage_used: StorageUsage,
    pub tokens: HashMap<AccountId, Balance>,
}

impl AccountDeposit {
    pub(crate) fn add(&mut self, token: &AccountId, amount: u128) {
        if let Some(x) = self.tokens.get_mut(token) {
            *x = *x + amount;
        } else {
            self.tokens.insert(token.clone(), amount);
        }
    }

    pub(crate) fn remove(&mut self, token: &AccountId, amount: u128) {
        if let Some(x) = self.tokens.get_mut(token) {
            assert!(*x >= amount, ERR13_NOT_ENOUGH_TOKENS_DEPOSITED);
            *x = *x - amount;
        } else {
            panic!(ERR14_NOT_ENOUGH_NEAR_DEPOSITED);
        }
    }

    #[inline]
    pub(crate) fn assert_storage(&self) {
        assert!(
            self.near >= (self.storage_used as u128) * STORAGE_PRICE_PER_BYTE,
            ERR21_ACC_STORAGE_TOO_LOW
        )
    }

    /// asserts that the account has anough NEAR to cover storage and use of `amout` NEAR.
    #[inline]
    pub(crate) fn assert_near(&self, amount: u128) {
        assert!(
            self.near >= amount + (self.storage_used as u128) * STORAGE_PRICE_PER_BYTE,
            ERR14_NOT_ENOUGH_NEAR_DEPOSITED,
        )
    }
}

// TODO:
// + finish storage tracking, example: https://github.com/robert-zaremba/vostok-dao/blob/master/src/lib.rs#L97
//   we don't do the storage refunds, instead we shold accumulate what storage has been used and keeping the following invariant all the time: account_deposit.amount >= account_deposit.storage  * STORAGE_PRICE_PER_BYTE
// +

// TODO make unit tests for AccountDeposit
#[cfg(test)]
mod tests {}
