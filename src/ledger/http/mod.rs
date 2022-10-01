use std::iter::FromIterator;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use sqlx::PgPool;
use tracing::error;
use uuid::Uuid;

use crate::{
    authentication::domain::session::{ExtractSession, Session},
    http_err::{ApiError, ApiResponse, ErrorRep},
    server::AppState,
};

use super::{
    commands::{postgres::PostgresCommands, TransactionCommands, UpdateTransactionError},
    domain,
    queries::{
        self, postgres::PostgresQueries, AccountQueries, CurrencyQueries, TransactionQueries,
    },
};

pub mod reps;

pub fn routes(app_state: AppState) -> Router<AppState> {
    Router::with_state(app_state)
        .route("/accounts", get(get_accounts))
        .route("/accounts/:account/balance", get(get_account_balance))
        .route(
            "/transactions",
            get(get_transactions).post(create_transaction),
        )
        .route(
            "/transactions/:transaction_id",
            get(get_transaction)
                .put(update_transaction)
                .delete(delete_transaction),
        )
}

async fn delete_transaction(
    session: Session,
    State(db): State<PgPool>,
    Path(transaction_id): Path<Uuid>,
) -> ApiResponse<StatusCode> {
    let commands = PostgresCommands(&db);

    match commands
        .delete_transaction(session.user_id(), transaction_id)
        .await
    {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(error) => {
            error!(?error, "Failed to delete transaction.");

            Err(ApiError::InternalServerError)
        }
    }
}

async fn get_account_balance(
    State(db): State<PgPool>,
    ExtractSession(session): ExtractSession,
    Path(account): Path<String>,
) -> ApiResponse<Json<Vec<reps::CurrencyAmount>>> {
    let queries = PostgresQueries(&db);

    match queries
        .get_account_balance(session.user_id(), account.to_owned())
        .await
    {
        Ok(balances) => Ok(Json(
            balances.iter().map(reps::CurrencyAmount::from).collect(),
        )),
        Err(error) => {
            error!(%account, ?error, "Failed to query for account balance.");

            Err(ApiError::InternalServerError)
        }
    }
}

#[derive(Deserialize)]
struct GetAccountsParams {
    query: Option<String>,
}

async fn get_accounts(
    State(db): State<PgPool>,
    session: Session,
    Query(query): Query<GetAccountsParams>,
) -> ApiResponse<Json<Vec<String>>> {
    let queries = PostgresQueries(&db);

    match queries
        .list_accounts_by_popularity(session.user_id(), query.query)
        .await
    {
        Ok(accounts) => Ok(Json(accounts)),
        Err(error) => {
            error!(?error, "Failed to list accounts.");

            Err(ApiError::InternalServerError)
        }
    }
}

pub enum GetTransactionResponse {
    Ok(reps::Transaction),
    NotFound(ErrorRep),
}

impl IntoResponse for GetTransactionResponse {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::Ok(transaction) => (StatusCode::OK, Json(transaction)).into_response(),
            Self::NotFound(error) => (StatusCode::NOT_FOUND, Json(error)).into_response(),
        }
    }
}

impl From<Option<domain::transactions::Transaction>> for GetTransactionResponse {
    fn from(transaction: Option<domain::transactions::Transaction>) -> Self {
        match transaction {
            Some(t) => Self::Ok((&t).into()),
            None => Self::NotFound(ErrorRep {
                message: "Transaction not found.".to_owned(),
            }),
        }
    }
}

async fn get_transaction(
    session: Session,
    State(db): State<PgPool>,
    Path(transaction_id): Path<Uuid>,
) -> Result<GetTransactionResponse, ApiError> {
    let queries = PostgresQueries(&db);

    match queries
        .get_transaction(session.user_id(), transaction_id)
        .await
    {
        Ok(transaction) => Ok(transaction.into()),
        Err(error) => {
            error!(?error, "Failed to query for transaction.");

            Err(ApiError::InternalServerError)
        }
    }
}

#[derive(Deserialize)]
struct GetTransactionsParams {
    account: Option<String>,
    after: Option<reps::EncodedTransactionCursor>,
}

async fn get_transactions(
    session: Session,
    State(db): State<PgPool>,
    Query(GetTransactionsParams { account, after }): Query<GetTransactionsParams>,
) -> ApiResponse<Json<reps::ResourceCollection<reps::Transaction, reps::EncodedTransactionCursor>>>
{
    let queries = PostgresQueries(&db);

    let query = queries::TransactionQuery {
        user_id: session.user_id(),
        after: after.as_ref().map(|c| (&c.0).into()),
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

            Err(ApiError::InternalServerError)
        }
    }
}

pub enum CreateTransactionResponse {
    Created(reps::Transaction),
    BadRequest(reps::TransactionValidationError),
}

impl IntoResponse for CreateTransactionResponse {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::Created(transaction) => (StatusCode::CREATED, Json(transaction)).into_response(),
            Self::BadRequest(error) => (StatusCode::BAD_REQUEST, Json(error)).into_response(),
        }
    }
}

impl From<&domain::transactions::Transaction> for CreateTransactionResponse {
    fn from(transaction: &domain::transactions::Transaction) -> Self {
        Self::Created(transaction.into())
    }
}

impl From<reps::TransactionValidationError> for CreateTransactionResponse {
    fn from(rep: reps::TransactionValidationError) -> Self {
        Self::BadRequest(rep)
    }
}

async fn create_transaction(
    session: Session,
    State(db): State<PgPool>,
    Json(new_transaction): Json<reps::NewTransaction>,
) -> ApiResponse<CreateTransactionResponse> {
    let queries = PostgresQueries(&db);

    let used_currency_codes = Vec::from_iter(new_transaction.used_currency_codes());
    let used_currencies = match queries.get_currencies_by_code(used_currency_codes).await {
        Ok(currencies) => currencies,
        Err(error) => {
            error!(?error, currency_codes = ?new_transaction.used_currency_codes(), "Failed to fetch currencies used in transaction.");

            return Err(ApiError::InternalServerError);
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

            return Err(ApiError::InternalServerError);
        }
    };

    Ok((&saved_transaction).into())
}

pub enum UpdateTransactionResponse {
    Updated(reps::Transaction),
    BadRequest(reps::TransactionValidationError),
    NotFound(ErrorRep),
}

impl IntoResponse for UpdateTransactionResponse {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::Updated(transaction) => (StatusCode::OK, Json(transaction)).into_response(),
            Self::BadRequest(error) => (StatusCode::BAD_REQUEST, Json(error)).into_response(),
            Self::NotFound(error) => (StatusCode::NOT_FOUND, Json(error)).into_response(),
        }
    }
}

impl From<&domain::transactions::Transaction> for UpdateTransactionResponse {
    fn from(transaction: &domain::transactions::Transaction) -> Self {
        Self::Updated(transaction.into())
    }
}

impl From<reps::TransactionValidationError> for UpdateTransactionResponse {
    fn from(rep: reps::TransactionValidationError) -> Self {
        Self::BadRequest(rep)
    }
}

async fn update_transaction(
    session: Session,
    State(db): State<PgPool>,
    Path(transaction_id): Path<Uuid>,
    Json(updated_transaction): Json<reps::NewTransaction>,
) -> ApiResponse<UpdateTransactionResponse> {
    let queries = PostgresQueries(&db);

    let used_currency_codes = Vec::from_iter(updated_transaction.used_currency_codes());
    let used_currencies = match queries.get_currencies_by_code(used_currency_codes).await {
        Ok(currencies) => currencies,
        Err(error) => {
            error!(?error, currency_codes = ?updated_transaction.used_currency_codes(), "Failed to fetch currencies used in transaction.");

            return Err(ApiError::InternalServerError);
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
            return Ok(UpdateTransactionResponse::NotFound(ErrorRep {
                message: "No transaction found with the provided ID.".to_owned(),
            }))
        }
        Err(error) => {
            error!(?error, %transaction_id, "Failed to update transaction.");

            return Err(ApiError::InternalServerError);
        }
    };

    Ok((&saved_transaction).into())
}
