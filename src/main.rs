use csv_async::{AsyncReaderBuilder, AsyncWriterBuilder};
use futures::StreamExt;
use rust_decimal::Decimal;
use serde::de::{self, Deserializer, MapAccess, Visitor};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fmt;
use std::process;
use tokio::fs::File;
use tokio::io;

// Strongly-typed transaction enum
#[derive(Debug, Clone)]
enum InputTransaction {
    Deposit {
        client: u16,
        tx: u32,
        amount: Decimal,
    },
    Withdrawal {
        client: u16,
        tx: u32,
        amount: Decimal,
    },
    Dispute {
        client: u16,
        tx: u32,
    },
    Resolve {
        client: u16,
        tx: u32,
    },
    Chargeback {
        client: u16,
        tx: u32,
    },
}

impl<'de> Deserialize<'de> for InputTransaction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "lowercase")]
        enum Field {
            Type,
            Client,
            Tx,
            Amount,
        }

        struct TransactionVisitor;

        impl<'de> Visitor<'de> for TransactionVisitor {
            type Value = InputTransaction;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a transaction record")
            }

            fn visit_map<V>(self, mut map: V) -> Result<InputTransaction, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut tx_type: Option<String> = None;
                let mut client: Option<u16> = None;
                let mut tx: Option<u32> = None;
                let mut amount: Option<Decimal> = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Type => {
                            if tx_type.is_some() {
                                return Err(de::Error::duplicate_field("type"));
                            }
                            tx_type = Some(map.next_value()?);
                        }
                        Field::Client => {
                            if client.is_some() {
                                return Err(de::Error::duplicate_field("client"));
                            }
                            client = Some(map.next_value()?);
                        }
                        Field::Tx => {
                            if tx.is_some() {
                                return Err(de::Error::duplicate_field("tx"));
                            }
                            tx = Some(map.next_value()?);
                        }
                        Field::Amount => {
                            amount = map.next_value()?;
                        }
                    }
                }

                let tx_type = tx_type.ok_or_else(|| de::Error::missing_field("type"))?;
                let client = client.ok_or_else(|| de::Error::missing_field("client"))?;
                let tx = tx.ok_or_else(|| de::Error::missing_field("tx"))?;

                match tx_type.trim().to_lowercase().as_str() {
                    "deposit" => {
                        let amount = amount.ok_or_else(|| de::Error::missing_field("amount"))?;
                        Ok(InputTransaction::Deposit { client, tx, amount })
                    }
                    "withdrawal" => {
                        let amount = amount.ok_or_else(|| de::Error::missing_field("amount"))?;
                        Ok(InputTransaction::Withdrawal { client, tx, amount })
                    }
                    "dispute" => Ok(InputTransaction::Dispute { client, tx }),
                    "resolve" => Ok(InputTransaction::Resolve { client, tx }),
                    "chargeback" => Ok(InputTransaction::Chargeback { client, tx }),
                    _ => Err(de::Error::unknown_variant(
                        &tx_type,
                        &["deposit", "withdrawal", "dispute", "resolve", "chargeback"],
                    )),
                }
            }
        }

        deserializer.deserialize_map(TransactionVisitor)
    }
}

#[derive(Debug, Clone, PartialEq)]
enum TransactionType {
    Deposit,
    Withdrawal,
}

// Internal representation of a stored transaction for dispute tracking
#[derive(Debug, Clone)]
struct StoredTransaction {
    #[allow(dead_code)]
    tx_id: u32,
    client_id: u16,
    tx_type: TransactionType,
    amount: Decimal,
    disputed: bool,
}

#[derive(Debug, Clone, Serialize)]
struct Account {
    client: u16,
    #[serde(serialize_with = "serialize_decimal")]
    available: Decimal,
    #[serde(serialize_with = "serialize_decimal")]
    held: Decimal,
    #[serde(serialize_with = "serialize_decimal")]
    total: Decimal,
    locked: bool,
}

fn serialize_decimal<S>(decimal: &Decimal, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let rounded = decimal.round_dp(4);
    serializer.serialize_str(&rounded.to_string())
}

impl Account {
    fn new(client_id: u16) -> Self {
        Account {
            client: client_id,
            available: Decimal::ZERO,
            held: Decimal::ZERO,
            total: Decimal::ZERO,
            locked: false,
        }
    }

    fn deposit(&mut self, amount: Decimal) {
        if !self.locked {
            self.available += amount;
            self.total += amount;
        }
    }

    fn withdraw(&mut self, amount: Decimal) -> bool {
        if !self.locked && self.available >= amount {
            self.available -= amount;
            self.total -= amount;
            true
        } else {
            false
        }
    }

    fn hold_funds(&mut self, amount: Decimal) {
        if !self.locked {
            self.available -= amount;
            self.held += amount;
        }
    }

    fn release_funds(&mut self, amount: Decimal) {
        if !self.locked && self.held >= amount {
            self.held -= amount;
            self.available += amount;
        }
    }

    fn chargeback(&mut self, amount: Decimal) {
        if self.held >= amount {
            self.held -= amount;
            self.total -= amount;
            self.locked = true;
        }
    }
}

struct PaymentsEngine {
    accounts: HashMap<u16, Account>,
    transactions: HashMap<u32, StoredTransaction>,
}

impl PaymentsEngine {
    fn new() -> Self {
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

    fn process_transaction(&mut self, input: InputTransaction) {
        match input {
            InputTransaction::Deposit { client, tx, amount } => {
                if amount > Decimal::ZERO {
                    let account = self.get_or_create_account(client);
                    account.deposit(amount);

                    // Store the transaction for potential disputes
                    self.transactions.insert(
                        tx,
                        StoredTransaction {
                            tx_id: tx,
                            client_id: client,
                            tx_type: TransactionType::Deposit,
                            amount,
                            disputed: false,
                        },
                    );
                }
            }
            InputTransaction::Withdrawal { client, tx, amount } => {
                if amount > Decimal::ZERO {
                    let account = self.get_or_create_account(client);
                    if account.withdraw(amount) {
                        // Only store successful withdrawals
                        self.transactions.insert(
                            tx,
                            StoredTransaction {
                                tx_id: tx,
                                client_id: client,
                                tx_type: TransactionType::Withdrawal,
                                amount,
                                disputed: false,
                            },
                        );
                    }
                }
            }
            InputTransaction::Dispute { client, tx } => {
                // Find the referenced transaction and check if it can be disputed
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
                // Find the referenced transaction and check if it can be resolved
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
                // Find the referenced transaction and check if it can be charged back
                let should_chargeback = self.transactions.get(&tx).map_or(false, |t| {
                    t.client_id == client && t.disputed
                });

                if should_chargeback {
                    if let Some(transaction) = self.transactions.get_mut(&tx) {
                        let amount = transaction.amount;
                        transaction.disputed = false; // No longer disputed, it's been charged back
                        let account = self.get_or_create_account(client);
                        account.chargeback(amount);
                    }
                }
            }
        }
    }

    fn get_accounts(&self) -> Vec<Account> {
        let mut accounts: Vec<_> = self.accounts.values().cloned().collect();
        accounts.sort_by_key(|a| a.client);
        accounts
    }
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        eprintln!("Usage: {} <transactions.csv>", args[0]);
        process::exit(1);
    }

    let input_file = &args[1];

    // Open the input file
    let file = File::open(input_file).await?;
    let mut reader = AsyncReaderBuilder::new()
        .flexible(true)
        .trim(csv_async::Trim::All)
        .create_deserializer(file);

    // Process transactions
    let mut engine = PaymentsEngine::new();
    let mut records = reader.deserialize::<InputTransaction>();

    while let Some(result) = records.next().await {
        match result {
            Ok(transaction) => {
                engine.process_transaction(transaction);
            }
            Err(_) => {
                // Skip invalid records silently
                continue;
            }
        }
    }

    // Write output to stdout
    let mut stdout = io::stdout();
    let mut writer = AsyncWriterBuilder::new().create_serializer(&mut stdout);

    for account in engine.get_accounts() {
        writer.serialize(account).await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_basic_deposit() {
        let mut engine = PaymentsEngine::new();
        engine.process_transaction(InputTransaction::Deposit {
            client: 1,
            tx: 1,
            amount: Decimal::from_str("100.0").unwrap(),
        });

        let accounts = engine.get_accounts();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].client, 1);
        assert_eq!(accounts[0].available, Decimal::from_str("100.0").unwrap());
        assert_eq!(accounts[0].total, Decimal::from_str("100.0").unwrap());
        assert_eq!(accounts[0].held, Decimal::ZERO);
    }

    #[test]
    fn test_basic_withdrawal() {
        let mut engine = PaymentsEngine::new();
        engine.process_transaction(InputTransaction::Deposit {
            client: 1,
            tx: 1,
            amount: Decimal::from_str("100.0").unwrap(),
        });
        engine.process_transaction(InputTransaction::Withdrawal {
            client: 1,
            tx: 2,
            amount: Decimal::from_str("30.0").unwrap(),
        });

        let accounts = engine.get_accounts();
        assert_eq!(accounts[0].available, Decimal::from_str("70.0").unwrap());
        assert_eq!(accounts[0].total, Decimal::from_str("70.0").unwrap());
    }

    #[test]
    fn test_insufficient_funds_withdrawal() {
        let mut engine = PaymentsEngine::new();
        engine.process_transaction(InputTransaction::Deposit {
            client: 1,
            tx: 1,
            amount: Decimal::from_str("50.0").unwrap(),
        });
        engine.process_transaction(InputTransaction::Withdrawal {
            client: 1,
            tx: 2,
            amount: Decimal::from_str("100.0").unwrap(),
        });

        let accounts = engine.get_accounts();
        // Withdrawal should fail, balance should remain 50.0
        assert_eq!(accounts[0].available, Decimal::from_str("50.0").unwrap());
        assert_eq!(accounts[0].total, Decimal::from_str("50.0").unwrap());
    }

    #[test]
    fn test_dispute() {
        let mut engine = PaymentsEngine::new();
        engine.process_transaction(InputTransaction::Deposit {
            client: 1,
            tx: 1,
            amount: Decimal::from_str("100.0").unwrap(),
        });
        engine.process_transaction(InputTransaction::Dispute { client: 1, tx: 1 });

        let accounts = engine.get_accounts();
        assert_eq!(accounts[0].available, Decimal::ZERO);
        assert_eq!(accounts[0].held, Decimal::from_str("100.0").unwrap());
        assert_eq!(accounts[0].total, Decimal::from_str("100.0").unwrap());
        assert!(!accounts[0].locked);
    }

    #[test]
    fn test_dispute_with_negative_available() {
        let mut engine = PaymentsEngine::new();
        engine.process_transaction(InputTransaction::Deposit {
            client: 1,
            tx: 1,
            amount: Decimal::from_str("100.0").unwrap(),
        });
        engine.process_transaction(InputTransaction::Withdrawal {
            client: 1,
            tx: 2,
            amount: Decimal::from_str("60.0").unwrap(),
        });
        engine.process_transaction(InputTransaction::Dispute { client: 1, tx: 1 });

        let accounts = engine.get_accounts();
        // available: 40 - 100 = -60
        assert_eq!(accounts[0].available, Decimal::from_str("-60.0").unwrap());
        assert_eq!(accounts[0].held, Decimal::from_str("100.0").unwrap());
        assert_eq!(accounts[0].total, Decimal::from_str("40.0").unwrap());
    }

    #[test]
    fn test_resolve() {
        let mut engine = PaymentsEngine::new();
        engine.process_transaction(InputTransaction::Deposit {
            client: 1,
            tx: 1,
            amount: Decimal::from_str("100.0").unwrap(),
        });
        engine.process_transaction(InputTransaction::Dispute { client: 1, tx: 1 });
        engine.process_transaction(InputTransaction::Resolve { client: 1, tx: 1 });

        let accounts = engine.get_accounts();
        assert_eq!(accounts[0].available, Decimal::from_str("100.0").unwrap());
        assert_eq!(accounts[0].held, Decimal::ZERO);
        assert_eq!(accounts[0].total, Decimal::from_str("100.0").unwrap());
        assert!(!accounts[0].locked);
    }

    #[test]
    fn test_chargeback() {
        let mut engine = PaymentsEngine::new();
        engine.process_transaction(InputTransaction::Deposit {
            client: 1,
            tx: 1,
            amount: Decimal::from_str("100.0").unwrap(),
        });
        engine.process_transaction(InputTransaction::Dispute { client: 1, tx: 1 });
        engine.process_transaction(InputTransaction::Chargeback { client: 1, tx: 1 });

        let accounts = engine.get_accounts();
        assert_eq!(accounts[0].available, Decimal::ZERO);
        assert_eq!(accounts[0].held, Decimal::ZERO);
        assert_eq!(accounts[0].total, Decimal::ZERO);
        assert!(accounts[0].locked);
    }

    #[test]
    fn test_locked_account_prevents_deposits() {
        let mut engine = PaymentsEngine::new();
        engine.process_transaction(InputTransaction::Deposit {
            client: 1,
            tx: 1,
            amount: Decimal::from_str("100.0").unwrap(),
        });
        engine.process_transaction(InputTransaction::Dispute { client: 1, tx: 1 });
        engine.process_transaction(InputTransaction::Chargeback { client: 1, tx: 1 });
        // Try to deposit after account is locked
        engine.process_transaction(InputTransaction::Deposit {
            client: 1,
            tx: 2,
            amount: Decimal::from_str("50.0").unwrap(),
        });

        let accounts = engine.get_accounts();
        // Balance should still be 0, deposit should be rejected
        assert_eq!(accounts[0].total, Decimal::ZERO);
        assert!(accounts[0].locked);
    }

    #[test]
    fn test_multiple_clients() {
        let mut engine = PaymentsEngine::new();
        engine.process_transaction(InputTransaction::Deposit {
            client: 1,
            tx: 1,
            amount: Decimal::from_str("100.0").unwrap(),
        });
        engine.process_transaction(InputTransaction::Deposit {
            client: 2,
            tx: 2,
            amount: Decimal::from_str("200.0").unwrap(),
        });

        let accounts = engine.get_accounts();
        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts[0].client, 1);
        assert_eq!(accounts[0].total, Decimal::from_str("100.0").unwrap());
        assert_eq!(accounts[1].client, 2);
        assert_eq!(accounts[1].total, Decimal::from_str("200.0").unwrap());
    }

    #[test]
    fn test_dispute_only_affects_deposits() {
        let mut engine = PaymentsEngine::new();
        engine.process_transaction(InputTransaction::Deposit {
            client: 1,
            tx: 1,
            amount: Decimal::from_str("100.0").unwrap(),
        });
        engine.process_transaction(InputTransaction::Withdrawal {
            client: 1,
            tx: 2,
            amount: Decimal::from_str("30.0").unwrap(),
        });
        // Try to dispute the withdrawal
        engine.process_transaction(InputTransaction::Dispute { client: 1, tx: 2 });

        let accounts = engine.get_accounts();
        // Dispute should be ignored, balance unchanged
        assert_eq!(accounts[0].available, Decimal::from_str("70.0").unwrap());
        assert_eq!(accounts[0].held, Decimal::ZERO);
    }

    #[test]
    fn test_precision() {
        let mut engine = PaymentsEngine::new();
        engine.process_transaction(InputTransaction::Deposit {
            client: 1,
            tx: 1,
            amount: Decimal::from_str("1.2345").unwrap(),
        });

        let accounts = engine.get_accounts();
        assert_eq!(accounts[0].available, Decimal::from_str("1.2345").unwrap());
    }

    #[test]
    fn test_cannot_resolve_without_dispute() {
        let mut engine = PaymentsEngine::new();
        engine.process_transaction(InputTransaction::Deposit {
            client: 1,
            tx: 1,
            amount: Decimal::from_str("100.0").unwrap(),
        });
        engine.process_transaction(InputTransaction::Resolve { client: 1, tx: 1 });

        let accounts = engine.get_accounts();
        // Resolve should be ignored without prior dispute
        assert_eq!(accounts[0].available, Decimal::from_str("100.0").unwrap());
        assert_eq!(accounts[0].held, Decimal::ZERO);
    }

    #[test]
    fn test_cannot_chargeback_without_dispute() {
        let mut engine = PaymentsEngine::new();
        engine.process_transaction(InputTransaction::Deposit {
            client: 1,
            tx: 1,
            amount: Decimal::from_str("100.0").unwrap(),
        });
        engine.process_transaction(InputTransaction::Chargeback { client: 1, tx: 1 });

        let accounts = engine.get_accounts();
        // Chargeback should be ignored without prior dispute
        assert_eq!(accounts[0].available, Decimal::from_str("100.0").unwrap());
        assert_eq!(accounts[0].total, Decimal::from_str("100.0").unwrap());
        assert!(!accounts[0].locked);
    }
}
