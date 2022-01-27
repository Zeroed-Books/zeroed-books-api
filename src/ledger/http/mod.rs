use std::{collections::HashSet, iter::FromIterator};

use chrono::{DateTime, NaiveDate, Utc};
use rocket::{http::Status, serde::json::Json, Route};
use serde::{Deserialize, Serialize};
use tracing::error;
use uuid::Uuid;

use crate::{
    authentication::domain::session::Session,
    http_err::{ApiError, ErrorRep, InternalServerError},
    PostgresConn,
};

use super::{
    commands::{postgres::PostgresCommands, TransactionCommands, UpdateTransactionError},
    domain::{self, transactions::NewTransactionError},
    queries::{postgres::PostgresQueries, CurrencyQueries, TransactionQueries},
};

pub mod reps;

pub fn routes() -> Vec<Route> {
    routes![
        create_transaction,
        delete_transaction,
        get_transaction,
        get_transactions,
        update_transaction,
    ]
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

#[delete("/transactions/<transaction_id>")]
async fn delete_transaction(
    session: Session,
    db: PostgresConn,
    transaction_id: Uuid,
) -> Result<Status, ApiError> {
    let commands = PostgresCommands(&db);

    match commands
        .delete_transaction(session.user_id(), transaction_id)
        .await
    {
        Ok(()) => Ok(Status::NoContent),
        Err(error) => {
            error!(?error, "Failed to delete transaction.");

            Err(InternalServerError::default().into())
        }
    }
}

#[derive(Responder)]
pub enum GetTransactionResponse {
    Ok(Json<Transaction>),
    #[response(status = 404)]
    NotFound(Json<ErrorRep>),
}

impl From<Option<domain::transactions::Transaction>> for GetTransactionResponse {
    fn from(transaction: Option<domain::transactions::Transaction>) -> Self {
        match transaction {
            Some(t) => Self::Ok(Json((&t).into())),
            None => Self::NotFound(Json(ErrorRep {
                message: "Transaction not found.".to_owned(),
            })),
        }
    }
}

#[get("/transactions/<transaction_id>")]
async fn get_transaction(
    session: Session,
    db: PostgresConn,
    transaction_id: Uuid,
) -> Result<GetTransactionResponse, ApiError> {
    let queries = PostgresQueries(&db);

    match queries
        .get_transaction(session.user_id(), transaction_id)
        .await
    {
        Ok(transaction) => Ok(transaction.into()),
        Err(error) => {
            error!(?error, "Failed to query for transaction.");

            Err(InternalServerError::default().into())
        }
    }
}

#[get("/transactions")]
async fn get_transactions(
    session: Session,
    db: PostgresConn,
) -> Result<Json<reps::ResourceCollection<Transaction>>, ApiError> {
    let queries = PostgresQueries(&db);

    match queries.latest_transactions(session.user_id()).await {
        Ok(transactions) => Ok(Json(reps::ResourceCollection {
            items: transactions
                .iter()
                .map(|transaction| transaction.into())
                .collect(),
        })),
        Err(error) => {
            error!(?error, "Failed to list transactions.");

            Err(InternalServerError::default().into())
        }
    }
}

#[derive(Responder)]
pub enum CreateTransactionResponse {
    #[response(status = 201)]
    Created(Json<Transaction>),
    #[response(status = 400)]
    BadRequest(Json<reps::TransactionValidationError>),
}

impl From<&domain::transactions::Transaction> for CreateTransactionResponse {
    fn from(transaction: &domain::transactions::Transaction) -> Self {
        Self::Created(Json(transaction.into()))
    }
}

impl From<reps::TransactionValidationError> for CreateTransactionResponse {
    fn from(rep: reps::TransactionValidationError) -> Self {
        Self::BadRequest(Json(rep))
    }
}

#[post("/transactions", data = "<new_transaction>")]
async fn create_transaction(
    session: Session,
    new_transaction: Json<NewTransaction>,
    db: PostgresConn,
) -> Result<CreateTransactionResponse, ApiError> {
    let queries = PostgresQueries(&db);

    let used_currency_codes = Vec::from_iter(new_transaction.used_currency_codes());
    let used_currencies = match queries.get_currencies_by_code(used_currency_codes).await {
        Ok(currencies) => currencies,
        Err(error) => {
            error!(?error, currency_codes = ?new_transaction.used_currency_codes(), "Failed to fetch currencies used in transaction.");

            return Err(InternalServerError::default().into());
        }
    };

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
                        Err(error) => {
                            return Ok(reps::TransactionValidationError::from(error).into())
                        }
                    }
                } else {
                    return Ok(reps::TransactionValidationError {
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
                reps::TransactionValidationError {
                    message: Some("The entries in the transaction are unbalanced.".to_owned()),
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

#[derive(Responder)]
pub enum UpdateTransactionResponse {
    #[response(status = 200)]
    Updated(Json<Transaction>),
    #[response(status = 400)]
    BadRequest(Json<reps::TransactionValidationError>),
    #[response(status = 404)]
    NotFound(Json<ErrorRep>),
}

impl From<&domain::transactions::Transaction> for UpdateTransactionResponse {
    fn from(transaction: &domain::transactions::Transaction) -> Self {
        Self::Updated(Json(transaction.into()))
    }
}

impl From<reps::TransactionValidationError> for UpdateTransactionResponse {
    fn from(rep: reps::TransactionValidationError) -> Self {
        Self::BadRequest(Json(rep))
    }
}

#[put("/transactions/<transaction_id>", data = "<updated_transaction>")]
async fn update_transaction(
    session: Session,
    transaction_id: Uuid,
    updated_transaction: Json<NewTransaction>,
    db: PostgresConn,
) -> Result<UpdateTransactionResponse, ApiError> {
    let queries = PostgresQueries(&db);

    let used_currency_codes = Vec::from_iter(updated_transaction.used_currency_codes());
    let used_currencies = match queries.get_currencies_by_code(used_currency_codes).await {
        Ok(currencies) => currencies,
        Err(error) => {
            error!(?error, currency_codes = ?updated_transaction.used_currency_codes(), "Failed to fetch currencies used in transaction.");

            return Err(InternalServerError::default().into());
        }
    };

    let mut parsed_entries = Vec::with_capacity(updated_transaction.entries.len());
    for new_entry in updated_transaction.entries.iter() {
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
                        Err(error) => {
                            return Ok(reps::TransactionValidationError::from(error).into())
                        }
                    }
                } else {
                    return Ok(reps::TransactionValidationError {
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
        updated_transaction.date,
        updated_transaction.payee.clone(),
        updated_transaction.notes.clone(),
        parsed_entries,
    ) {
        Ok(t) => t,
        Err(NewTransactionError::Unbalanced(_)) => {
            return Ok(reps::TransactionValidationError {
                message: Some("The entries in the transaction are unbalanced.".to_string()),
            }
            .into())
        }
    };

    let ledger_commands = PostgresCommands(&db);

    let saved_transaction = match ledger_commands
        .update_transaction(transaction_id, transaction)
        .await
    {
        Ok(t) => t,
        Err(UpdateTransactionError::TransactionNotFound) => {
            return Ok(UpdateTransactionResponse::NotFound(Json(ErrorRep {
                message: "No transaction found with the provided ID.".to_owned(),
            })))
        }
        Err(error) => {
            error!(?error, %transaction_id, "Failed to update transaction.");

            return Err(InternalServerError::default().into());
        }
    };

    Ok((&saved_transaction).into())
}
