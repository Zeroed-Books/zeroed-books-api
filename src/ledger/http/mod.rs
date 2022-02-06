use std::iter::FromIterator;

use rocket::{http::Status, serde::json::Json, Route};
use tracing::error;
use uuid::Uuid;

use crate::{
    authentication::domain::session::Session,
    http_err::{ApiError, ErrorRep, InternalServerError},
    PostgresConn,
};

use super::{
    commands::{postgres::PostgresCommands, TransactionCommands, UpdateTransactionError},
    domain,
    queries::{self, postgres::PostgresQueries, CurrencyQueries, TransactionQueries},
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
    Ok(Json<reps::Transaction>),
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

#[get("/transactions?<after>&<account>")]
async fn get_transactions(
    session: Session,
    db: PostgresConn,
    account: Option<&'_ str>,
    after: Option<reps::TransactionCursor>,
) -> Result<
    Json<reps::ResourceCollection<reps::Transaction, reps::EncodedTransactionCursor>>,
    ApiError,
> {
    let queries = PostgresQueries(&db);

    let query = queries::TransactionQuery {
        user_id: session.user_id(),
        after: after.as_ref().map(Into::into),
        account: account.map(String::from),
    };
    match queries.list_transactions(query).await {
        Ok(transactions) => Ok(Json(reps::ResourceCollection {
            next: transactions.next.map(Into::into),
            items: transactions
                .items
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
    Created(Json<reps::Transaction>),
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
    new_transaction: Json<reps::NewTransaction>,
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

    let transaction = match new_transaction.try_into_domain(session.user_id(), used_currencies) {
        Ok(t) => t,
        Err(error) => return Ok(error.into()),
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
    Updated(Json<reps::Transaction>),
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
    updated_transaction: Json<reps::NewTransaction>,
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

    let transaction = match updated_transaction.try_into_domain(session.user_id(), used_currencies)
    {
        Ok(t) => t,
        Err(error) => return Ok(error.into()),
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
