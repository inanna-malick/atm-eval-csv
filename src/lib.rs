//! Payments processing engine for deposits, withdrawals, disputes, and chargebacks.

pub mod transaction;
pub mod types;

use csv_async::{AsyncReaderBuilder, AsyncWriterBuilder};
use futures::StreamExt;
use serde::Serialize;
use std::collections::HashMap;
use tokio::fs::File;
use tokio::io;
use transaction::InputTransaction;
use types::{SignedDecimal, UnsignedDecimal};

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    AccountLocked,
    AccountNotFound,
    InsufficientFunds,
    InsufficientHeldFunds,
    DuplicateTransaction,
    TransactionNotFound,
    DisputeOnWithdrawal,
    AlreadyDisputed,
    ResolveNonDisputed,
    ChargebackNonDisputed,
}

fn serialize_decimal<S, T: std::fmt::Display>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&format!("{:.4}", value))
}

#[derive(Debug, Clone, Serialize)]
pub struct Account {
    pub client: u16,
    #[serde(serialize_with = "serialize_decimal")]
    pub available: SignedDecimal,
    #[serde(serialize_with = "serialize_decimal")]
    pub held: UnsignedDecimal,
    #[serde(serialize_with = "serialize_decimal")]
    pub total: SignedDecimal,
    pub locked: bool,
}

#[derive(Debug, Clone)]
pub enum StoredTransaction {
    Deposit {
        amount: UnsignedDecimal,
        disputed: bool,
    },
    Withdrawal {
        amount: UnsignedDecimal,
    },
}

#[derive(Default)]
pub struct PaymentsEngine {
    pub accounts: HashMap<u16, Account>,
    transactions: HashMap<(u16, u32), StoredTransaction>,
}

impl PaymentsEngine {
    pub fn process_transaction(&mut self, input: InputTransaction) -> Result<(), Error> {
        match input {
            InputTransaction::Deposit { client, tx, amount } => {
                if self.transactions.contains_key(&(client, tx)) {
                    return Err(Error::DuplicateTransaction);
                }
                let account = self.accounts.entry(client).or_insert_with(|| Account {
                    client,
                    available: SignedDecimal::ZERO,
                    held: UnsignedDecimal::ZERO,
                    total: SignedDecimal::ZERO,
                    locked: false,
                });
                if account.locked {
                    return Err(Error::AccountLocked);
                }
                let amount_signed = SignedDecimal::from(amount);
                account.available += amount_signed;
                account.total += amount_signed;
                self.transactions.insert(
                    (client, tx),
                    StoredTransaction::Deposit {
                        amount,
                        disputed: false,
                    },
                );
                Ok(())
            }
            InputTransaction::Withdrawal { client, tx, amount } => {
                if self.transactions.contains_key(&(client, tx)) {
                    return Err(Error::DuplicateTransaction);
                }
                let account = self
                    .accounts
                    .get_mut(&client)
                    .ok_or(Error::AccountNotFound)?;
                if account.locked {
                    return Err(Error::AccountLocked);
                }
                let amount_signed = SignedDecimal::from(amount);
                if account.available < amount_signed {
                    return Err(Error::InsufficientFunds);
                }
                account.available -= amount_signed;
                account.total -= amount_signed;
                self.transactions
                    .insert((client, tx), StoredTransaction::Withdrawal { amount });
                Ok(())
            }
            InputTransaction::Dispute { client, tx } => {
                let account = self
                    .accounts
                    .get_mut(&client)
                    .ok_or(Error::TransactionNotFound)?;
                if account.locked {
                    return Err(Error::AccountLocked);
                }

                match self.transactions.get_mut(&(client, tx)) {
                    Some(StoredTransaction::Deposit { amount, disputed }) if !*disputed => {
                        let amt = *amount;
                        *disputed = true;
                        let amount_signed = SignedDecimal::from(amt);
                        account.available -= amount_signed;
                        account.held += amt;
                        Ok(())
                    }
                    Some(StoredTransaction::Deposit { .. }) => Err(Error::AlreadyDisputed),
                    Some(StoredTransaction::Withdrawal { .. }) => Err(Error::DisputeOnWithdrawal),
                    None => Err(Error::TransactionNotFound),
                }
            }
            InputTransaction::Resolve { client, tx } => {
                let account = self
                    .accounts
                    .get_mut(&client)
                    .ok_or(Error::TransactionNotFound)?;
                if account.locked {
                    return Err(Error::AccountLocked);
                }

                match self.transactions.get_mut(&(client, tx)) {
                    Some(StoredTransaction::Deposit { amount, disputed }) if *disputed => {
                        let amt = *amount;
                        *disputed = false;
                        if account.held < amt {
                            return Err(Error::InsufficientHeldFunds);
                        }
                        account.held -= amt;
                        let amount_signed = SignedDecimal::from(amt);
                        account.available += amount_signed;
                        Ok(())
                    }
                    Some(StoredTransaction::Deposit { .. }) => Err(Error::ResolveNonDisputed),
                    Some(StoredTransaction::Withdrawal { .. }) | None => {
                        Err(Error::TransactionNotFound)
                    }
                }
            }
            InputTransaction::Chargeback { client, tx } => {
                let account = self
                    .accounts
                    .get_mut(&client)
                    .ok_or(Error::TransactionNotFound)?;

                match self.transactions.get_mut(&(client, tx)) {
                    Some(StoredTransaction::Deposit { amount, disputed }) if *disputed => {
                        let amt = *amount;
                        *disputed = false;
                        if account.held < amt {
                            return Err(Error::InsufficientHeldFunds);
                        }
                        account.held -= amt;
                        let amount_signed = SignedDecimal::from(amt);
                        account.total -= amount_signed;
                        account.locked = true;
                        Ok(())
                    }
                    Some(StoredTransaction::Deposit { .. }) => Err(Error::ChargebackNonDisputed),
                    Some(StoredTransaction::Withdrawal { .. }) | None => {
                        Err(Error::TransactionNotFound)
                    }
                }
            }
        }
    }
}

pub async fn process_csv_file(
    input_path: &str,
) -> Result<PaymentsEngine, Box<dyn std::error::Error>> {
    let file = File::open(input_path).await?;
    let mut reader = AsyncReaderBuilder::new()
        .flexible(true)
        .trim(csv_async::Trim::All)
        .create_deserializer(file);

    let mut engine = PaymentsEngine::default();
    let mut records = reader.deserialize::<InputTransaction>();

    while let Some(result) = records.next().await {
        match result {
            Ok(transaction) => {
                if let Err(e) = engine.process_transaction(transaction) {
                    eprintln!("Transaction error: {:?}", e);
                }
            }
            Err(e) => {
                eprintln!("Parse error: {:?}", e);
            }
        }
    }

    Ok(engine)
}

pub async fn write_accounts_csv(
    accounts: &HashMap<u16, Account>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut stdout = io::stdout();
    let mut writer = AsyncWriterBuilder::new().create_serializer(&mut stdout);

    for account in accounts.values() {
        writer.serialize(account).await?;
    }

    Ok(())
}
