use rust_decimal::Decimal;
use serde::de::{self, Deserializer, MapAccess, Visitor};
use serde::Deserialize;
use std::fmt;

/// Strongly-typed transaction enum representing all possible transaction types
#[derive(Debug, Clone)]
pub enum InputTransaction {
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

impl InputTransaction {
    pub fn client(&self) -> u16 {
        match self {
            InputTransaction::Deposit { client, .. } => *client,
            InputTransaction::Withdrawal { client, .. } => *client,
            InputTransaction::Dispute { client, .. } => *client,
            InputTransaction::Resolve { client, .. } => *client,
            InputTransaction::Chargeback { client, .. } => *client,
        }
    }

    pub fn tx(&self) -> u32 {
        match self {
            InputTransaction::Deposit { tx, .. } => *tx,
            InputTransaction::Withdrawal { tx, .. } => *tx,
            InputTransaction::Dispute { tx, .. } => *tx,
            InputTransaction::Resolve { tx, .. } => *tx,
            InputTransaction::Chargeback { tx, .. } => *tx,
        }
    }
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
