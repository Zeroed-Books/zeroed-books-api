use std::{
    collections::{HashMap, HashSet},
    convert::TryInto,
    iter::FromIterator,
};

use chrono::{DateTime, NaiveDate, Utc};
use rocket::{serde::json::Json, Route};
use serde::{Deserialize, Serialize};
use tracing::error;
use uuid::Uuid;

use crate::{
    authentication::domain::session::Session,
    http_err::{ApiError, InternalServerError},
    PostgresConn,
};

use super::{
    commands::{postgres::PostgresCommands, Commands},
    domain::{self, currency::CurrencyParseError, transactions::NewTransactionError},
    models,
};

pub mod reps;

pub fn routes() -> Vec<Route> {
    routes![create_transaction]
}

#[derive(Deserialize)]
pub struct NewTransaction {
    pub date: chrono::NaiveDate,
    pub payee: String,
    pub notes: Option<String>,
    pub entries: Vec<NewTransactionEntry>,
}

impl NewTransaction {
    pub fn used_currency_codes(&self) -> HashSet<String> {
        self.entries
            .iter()
            .filter_map(|entry| {
                entry
                    .amount
                    .as_ref()
                    .map(|amount| amount.currency.to_owned())
            })
            .collect()
    }
}

#[derive(Deserialize)]
pub struct NewTransactionEntry {
    pub account: String,
    pub amount: Option<reps::CurrencyAmount>,
}

#[derive(Serialize)]
pub struct Transaction {
    pub id: Uuid,
    pub date: NaiveDate,
    pub payee: String,
    pub notes: String,
    pub entries: Vec<TransactionEntry>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<&domain::transactions::Transaction> for Transaction {
    fn from(domain: &domain::transactions::Transaction) -> Self {
        Self {
            id: domain.id,
            date: domain.date,
            payee: domain.payee.to_owned(),
            notes: domain.notes.to_owned(),
            entries: domain.entries.iter().map(|entry| entry.into()).collect(),
            created_at: domain.created_at,
            updated_at: domain.updated_at,
        }
    }
}

#[derive(Serialize)]
pub struct TransactionEntry {
    pub account: String,
    pub amount: reps::CurrencyAmount,
}

impl From<&domain::transactions::TransactionEntry> for TransactionEntry {
    fn from(domain: &domain::transactions::TransactionEntry) -> Self {
        Self {
            account: domain.account().to_string(),
            amount: domain.amount().into(),
        }
    }
}

#[derive(Serialize)]
pub struct TransactionErrorResponse {
    pub message: Option<String>,
}

#[derive(Responder)]
pub enum CreateTransactionResponse {
    #[response(status = 201)]
    Created(Json<Transaction>),
    #[response(status = 400)]
    BadRequest(Json<TransactionErrorResponse>),
}

impl From<&domain::transactions::Transaction> for CreateTransactionResponse {
    fn from(transaction: &domain::transactions::Transaction) -> Self {
        Self::Created(Json(transaction.into()))
    }
}

impl From<TransactionErrorResponse> for CreateTransactionResponse {
    fn from(response: TransactionErrorResponse) -> Self {
        Self::BadRequest(Json(response))
    }
}

#[post("/transactions", data = "<new_transaction>")]
async fn create_transaction(
    session: Session,
    new_transaction: Json<NewTransaction>,
    db: PostgresConn,
) -> Result<CreateTransactionResponse, ApiError> {
    let used_currency_codes = Vec::from_iter(new_transaction.used_currency_codes());

    let currencies: Vec<models::Currency> = match db
        .run(|conn| models::Currency::find_by_codes(conn, used_currency_codes))
        .await
    {
        Ok(c) => c,
        Err(error) => {
            error!(?error, "Failed to query for currency codes.");

            return Err(InternalServerError::default().into());
        }
    };

    let mut used_currencies = HashMap::new();
    for currency_model in currencies {
        let currency: domain::currency::Currency = match (&currency_model).try_into() {
            Ok(currency) => currency,
            Err(error) => {
                error!(?error, "Failed to convert currency model to domain object.");

                return Err(InternalServerError::default().into());
            }
        };

        used_currencies.insert(currency.code().to_owned(), currency);
    }

    let mut parsed_entries = Vec::with_capacity(new_transaction.entries.len());
    for new_entry in new_transaction.entries.iter() {
        let parsed_amount = match &new_entry.amount {
            None => None,
            Some(amount_rep) => {
                if let Some(currency) = used_currencies.get(&amount_rep.currency) {
                    let parse_result = domain::currency::CurrencyAmount::from_str(
                        currency.clone(),
                        &amount_rep.value,
                    );

                    match parse_result {
                            Ok(amount) => Some(amount),
                            Err(CurrencyParseError::InvalidNumber(raw_amount)) => return Ok(
                                TransactionErrorResponse {
                                    message: Some(format!("The amount '{}' is not a valid number.", raw_amount))
                                }.into()
                            ),
                            Err(CurrencyParseError::TooManyDecimals(decimals)) => return Ok(
                                TransactionErrorResponse {
                                    message: Some(format!("The currency allows {} decimal place(s), but the provided value had {}.", currency.minor_units(), decimals))
                                }.into()
                            )
                        }
                } else {
                    return Ok(TransactionErrorResponse {
                        message: Some(format!(
                            "The currency code '{}' is unrecognized.",
                            &amount_rep.currency
                        )),
                    }
                    .into());
                }
            }
        };

        parsed_entries.push(domain::transactions::NewTransactionEntry {
            account: new_entry.account.clone(),
            amount: parsed_amount,
        });
    }

    let transaction = match domain::transactions::NewTransaction::new(
        session.user_id(),
        new_transaction.date,
        new_transaction.payee.clone(),
        new_transaction.notes.clone(),
        parsed_entries,
    ) {
        Ok(t) => t,
        Err(NewTransactionError::Unbalanced(_)) => {
            return Ok(CreateTransactionResponse::BadRequest(Json(
                TransactionErrorResponse {
                    message: Some("The entries in the transaction are unbalanced.".to_string()),
                },
            )))
        }
    };

    let ledger_commands = PostgresCommands(&db);

    let saved_transaction = match ledger_commands.persist_transaction(transaction).await {
        Ok(t) => t,
        Err(error) => {
            error!(?error, "Failed to persist transaction.");

            return Err(InternalServerError::default().into());
        }
    };

    Ok((&saved_transaction).into())
}
