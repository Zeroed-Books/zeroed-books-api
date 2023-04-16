use std::collections::HashMap;

use axum::{
    extract::{FromRef, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use axum_jwks::Claims;
use chrono::NaiveDate;
use serde::Deserialize;
use tracing::{debug, error};
use uuid::Uuid;

use crate::{
    authentication::TokenClaims,
    database::PostgresConnection,
    http_err::{ApiError, ApiResponse, ErrorRep},
    ledger::{
        domain::transactions::{NewTransaction, NewTransactionData},
        queries::ReportInterval,
        services::{AccountBalanceType, LedgerService},
    },
    repos::transactions::TransactionQuery,
    server::AppState,
};

use crate::ledger::{
    commands::{postgres::PostgresCommands, TransactionCommands, UpdateTransactionError},
    domain,
    queries::{postgres::PostgresQueries, AccountQueries, TransactionQueries},
};

use super::reps::{self, PeriodicAccountBalances};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/accounts", get(get_accounts))
        .route("/accounts/:account/balance", get(get_account_balance))
        .route(
            "/accounts/:account/balance/monthly",
            get(get_account_balance_monthly),
        )
        .route(
            "/accounts/:account/balance/periodic",
            get(get_account_balance_periodic),
        )
        .route("/active-accounts", get(get_active_accounts))
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
    Claims(claims): Claims<TokenClaims>,
    State(app_state): State<AppState>,
    Path(transaction_id): Path<Uuid>,
) -> ApiResponse<StatusCode> {
    let db = PostgresConnection::from_ref(&app_state);
    let commands = PostgresCommands(&db);

    match commands
        .delete_transaction(claims.user_id(), transaction_id)
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
    Claims(claims): Claims<TokenClaims>,
    State(db): State<PostgresConnection>,
    Path(account): Path<String>,
) -> ApiResponse<Json<Vec<reps::CurrencyAmount>>> {
    let queries = PostgresQueries(db);

    match queries
        .get_account_balance(claims.user_id(), account.to_owned())
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

async fn get_account_balance_monthly(
    Claims(claims): Claims<TokenClaims>,
    State(ledger_service): State<LedgerService>,
    Path(account): Path<String>,
) -> ApiResponse<Json<HashMap<NaiveDate, Vec<reps::CurrencyAmount>>>> {
    match ledger_service
        .get_monthly_account_balance(claims.user_id(), &account)
        .await
    {
        Ok(balances) => Ok(Json(
            balances
                .iter()
                .map(|(month, amounts)| {
                    (
                        month.to_owned(),
                        amounts.iter().map(reps::CurrencyAmount::from).collect(),
                    )
                })
                .collect(),
        )),
        Err(error) => {
            error!(
                user_id = claims.user_id(),
                ?error,
                "Failed to query for monthly account balances."
            );

            Err(ApiError::InternalServerError)
        }
    }
}

async fn get_account_balance_periodic(
    Claims(claims): Claims<TokenClaims>,
    State(ledger_service): State<LedgerService>,
    Path(account): Path<String>,
    Query(params): Query<PeriodicAccountBalanceParams>,
) -> ApiResponse<Json<PeriodicAccountBalances>> {
    let interval = match params.interval.as_deref() {
        None => ReportInterval::Monthly,
        Some("daily") => ReportInterval::Daily,
        Some("monthly") => ReportInterval::Monthly,
        Some("weekly") => ReportInterval::Weekly,
        _ => {
            return Err(ApiError::BadRequestReason(
                "Valid intervals are 'daily', 'monthly', or 'weekly'.".to_owned(),
            ))
        }
    };

    debug!(%account, ?interval, "Generating report of periodic monthly balance.");

    match ledger_service
        .account_periodic_balance(
            claims.user_id(),
            &account,
            AccountBalanceType::Cummulative,
            interval,
        )
        .await
    {
        Ok(balances) => Ok(Json(balances.into())),
        Err(error) => {
            error!(
                user_id = claims.user_id(),
                ?error,
                "Failed to query for periodic account balances."
            );

            Err(ApiError::InternalServerError)
        }
    }
}

#[derive(Deserialize)]
struct PeriodicAccountBalanceParams {
    interval: Option<String>,
}

#[derive(Deserialize)]
struct GetAccountsParams {
    query: Option<String>,
}

async fn get_accounts(
    Claims(claims): Claims<TokenClaims>,
    State(db): State<PostgresConnection>,
    Query(query): Query<GetAccountsParams>,
) -> ApiResponse<Json<Vec<String>>> {
    let queries = PostgresQueries(db);

    match queries
        .list_accounts_by_popularity(claims.user_id(), query.query)
        .await
    {
        Ok(accounts) => Ok(Json(accounts)),
        Err(error) => {
            error!(?error, "Failed to list accounts.");

            Err(ApiError::InternalServerError)
        }
    }
}

async fn get_active_accounts(
    Claims(claims): Claims<TokenClaims>,
    State(ledger_service): State<LedgerService>,
) -> ApiResponse<Json<Vec<String>>> {
    match ledger_service.list_active_accounts(claims.user_id()).await {
        Ok(accounts) => Ok(Json(accounts)),
        Err(error) => {
            error!(
                ?error,
                user_id = claims.user_id(),
                "Failed to list active accounts for user."
            );

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
    Claims(claims): Claims<TokenClaims>,
    State(db): State<PostgresConnection>,
    Path(transaction_id): Path<Uuid>,
) -> Result<GetTransactionResponse, ApiError> {
    let queries = PostgresQueries(db);

    match queries
        .get_transaction(claims.user_id(), transaction_id)
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
    Claims(claims): Claims<TokenClaims>,
    State(ledger_service): State<LedgerService>,
    Query(GetTransactionsParams { account, after }): Query<GetTransactionsParams>,
) -> ApiResponse<Json<reps::ResourceCollection<reps::Transaction, reps::EncodedTransactionCursor>>>
{
    let query = TransactionQuery {
        user_id: claims.user_id().to_owned(),
        after: after.as_ref().map(|c| (&c.0).into()),
        account: account.map(String::from),
    };
    match ledger_service.list_transactions(query).await {
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

async fn create_transaction(
    Claims(claims): Claims<TokenClaims>,
    State(db): State<PostgresConnection>,
    Json(new_transaction_data): Json<NewTransactionData>,
) -> ApiResponse<(StatusCode, Json<reps::Transaction>)> {
    let new_transaction = NewTransaction::from_data(claims.user_id(), new_transaction_data)?;

    let ledger_commands = PostgresCommands(&db);

    let saved_transaction = match ledger_commands.persist_transaction(new_transaction).await {
        Ok(t) => t,
        Err(error) => {
            error!(?error, "Failed to persist transaction.");

            return Err(ApiError::InternalServerError);
        }
    };

    Ok((
        StatusCode::CREATED,
        Json(reps::Transaction::from(&saved_transaction)),
    ))
}

pub enum UpdateTransactionResponse {
    Updated(reps::Transaction),
    NotFound(ErrorRep),
}

impl IntoResponse for UpdateTransactionResponse {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::Updated(transaction) => (StatusCode::OK, Json(transaction)).into_response(),
            Self::NotFound(error) => (StatusCode::NOT_FOUND, Json(error)).into_response(),
        }
    }
}

async fn update_transaction(
    Claims(claims): Claims<TokenClaims>,
    State(db): State<PostgresConnection>,
    Path(transaction_id): Path<Uuid>,
    Json(updated_transaction_data): Json<NewTransactionData>,
) -> ApiResponse<UpdateTransactionResponse> {
    let updated_transaction =
        NewTransaction::from_data(claims.user_id(), updated_transaction_data)?;

    let ledger_commands = PostgresCommands(&db);

    let saved_transaction = match ledger_commands
        .update_transaction(transaction_id, updated_transaction)
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

    Ok(UpdateTransactionResponse::Updated(reps::Transaction::from(
        &saved_transaction,
    )))
}
