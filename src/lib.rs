//! Payments processing engine for deposits, withdrawals, disputes, and chargebacks.

pub mod account;
pub mod transaction;

use account::{Account, StoredTransaction, TransactionType};
use csv_async::{AsyncReaderBuilder, AsyncWriterBuilder};
use fastnum::D128;
use futures::StreamExt;
use std::collections::HashMap;
use tokio::fs::File;
use tokio::io;
use transaction::InputTransaction;

type Decimal = D128;

pub struct PaymentsEngine {
    accounts: HashMap<u16, Account>,
    transactions: HashMap<u32, StoredTransaction>,
}

impl PaymentsEngine {
    pub fn new() -> Self {
        PaymentsEngine {
            accounts: HashMap::new(),
            transactions: HashMap::new(),
        }
    }

    fn get_or_create_account(&mut self, client_id: u16) -> &mut Account {
        self.accounts
            .entry(client_id)
            .or_insert_with(|| Account::new(client_id))
    }

    pub fn process_transaction(&mut self, input: InputTransaction) {
        match input {
            InputTransaction::Deposit { client, tx, amount } => {
                if amount > Decimal::ZERO {
                    let account = self.get_or_create_account(client);
                    account.deposit(amount);
                    self.transactions.insert(
                        tx,
                        StoredTransaction::new(tx, client, TransactionType::Deposit, amount),
                    );
                }
            }
            InputTransaction::Withdrawal { client, tx, amount } => {
                if amount > Decimal::ZERO {
                    let account = self.get_or_create_account(client);
                    if account.withdraw(amount) {
                        self.transactions.insert(
                            tx,
                            StoredTransaction::new(tx, client, TransactionType::Withdrawal, amount),
                        );
                    }
                }
            }
            InputTransaction::Dispute { client, tx } => {
                let should_dispute = self.transactions.get(&tx).map_or(false, |t| {
                    t.client_id == client
                        && t.tx_type == TransactionType::Deposit
                        && !t.disputed
                });

                if should_dispute {
                    if let Some(transaction) = self.transactions.get_mut(&tx) {
                        let amount = transaction.amount;
                        transaction.disputed = true;
                        let account = self.get_or_create_account(client);
                        account.hold_funds(amount);
                    }
                }
            }
            InputTransaction::Resolve { client, tx } => {
                let should_resolve = self.transactions.get(&tx).map_or(false, |t| {
                    t.client_id == client && t.disputed
                });

                if should_resolve {
                    if let Some(transaction) = self.transactions.get_mut(&tx) {
                        let amount = transaction.amount;
                        transaction.disputed = false;
                        let account = self.get_or_create_account(client);
                        account.release_funds(amount);
                    }
                }
            }
            InputTransaction::Chargeback { client, tx } => {
                let should_chargeback = self.transactions.get(&tx).map_or(false, |t| {
                    t.client_id == client && t.disputed
                });

                if should_chargeback {
                    if let Some(transaction) = self.transactions.get_mut(&tx) {
                        let amount = transaction.amount;
                        transaction.disputed = false;
                        let account = self.get_or_create_account(client);
                        account.chargeback(amount);
                    }
                }
            }
        }
    }

    pub fn get_accounts(&self) -> Vec<Account> {
        let mut accounts: Vec<_> = self.accounts.values().cloned().collect();
        accounts.sort_by_key(|a| a.client);
        accounts
    }
}

impl Default for PaymentsEngine {
    fn default() -> Self {
        Self::new()
    }
}

pub async fn process_csv_file(input_path: &str) -> Result<PaymentsEngine, Box<dyn std::error::Error>> {
    let file = File::open(input_path).await?;
    let mut reader = AsyncReaderBuilder::new()
        .flexible(true)
        .trim(csv_async::Trim::All)
        .create_deserializer(file);

    let mut engine = PaymentsEngine::new();
    let mut records = reader.deserialize::<InputTransaction>();

    while let Some(result) = records.next().await {
        match result {
            Ok(transaction) => {
                engine.process_transaction(transaction);
            }
            Err(_) => continue,
        }
    }

    Ok(engine)
}

pub async fn write_accounts_csv(accounts: &[Account]) -> Result<(), Box<dyn std::error::Error>> {
    let mut stdout = io::stdout();
    let mut writer = AsyncWriterBuilder::new().create_serializer(&mut stdout);

    for account in accounts {
        writer.serialize(account).await?;
    }

    Ok(())
}
