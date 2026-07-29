use super::{AppState, common};
use crate::{
    domain::AccountType,
    error::{AppError, AppResult},
    models::Account,
};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::get,
};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct AccountInput {
    name: String,
    institution: Option<String>,
    account_type: AccountType,
    base_currency: String,
    #[serde(default = "default_true")]
    include_in_net_worth: bool,
}

#[derive(Debug, Deserialize)]
pub struct AccountStatusInput {
    is_active: bool,
}

fn default_true() -> bool {
    true
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/accounts", get(list_accounts).post(create_account))
        .route(
            "/accounts/{id}",
            get(get_account)
                .put(replace_account)
                .patch(set_account_status)
                .delete(delete_account),
        )
}

async fn list_accounts(State(state): State<AppState>) -> AppResult<Json<Vec<Account>>> {
    let accounts = sqlx::query_as::<_, Account>(
        "SELECT * FROM accounts ORDER BY is_active DESC, lower(name), id",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(accounts))
}

async fn get_account(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<Account>> {
    Ok(Json(
        fetch_account(&state, &common::id(&id, "账户 ID")?).await?,
    ))
}

async fn create_account(
    State(state): State<AppState>,
    Json(input): Json<AccountInput>,
) -> AppResult<(StatusCode, Json<Account>)> {
    let values = validate(input)?;
    let id = Uuid::now_v7().to_string();

    sqlx::query(
        r#"INSERT INTO accounts
           (id, name, institution, account_type, base_currency, include_in_net_worth)
           VALUES (?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&id)
    .bind(values.name)
    .bind(values.institution)
    .bind(values.account_type.as_str())
    .bind(values.base_currency)
    .bind(values.include_in_net_worth)
    .execute(&state.db)
    .await?;

    Ok((StatusCode::CREATED, Json(fetch_account(&state, &id).await?)))
}

async fn replace_account(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<AccountInput>,
) -> AppResult<Json<Account>> {
    let id = common::id(&id, "账户 ID")?;
    let values = validate(input)?;

    let result = sqlx::query(
        r#"UPDATE accounts
           SET name = ?, institution = ?, account_type = ?, base_currency = ?,
               include_in_net_worth = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
           WHERE id = ?"#,
    )
    .bind(values.name)
    .bind(values.institution)
    .bind(values.account_type.as_str())
    .bind(values.base_currency)
    .bind(values.include_in_net_worth)
    .bind(&id)
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("账户不存在".to_owned()));
    }
    Ok(Json(fetch_account(&state, &id).await?))
}

async fn set_account_status(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<AccountStatusInput>,
) -> AppResult<Json<Account>> {
    let id = common::id(&id, "账户 ID")?;
    let result = sqlx::query(
        r#"UPDATE accounts
           SET is_active = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
           WHERE id = ?"#,
    )
    .bind(input.is_active)
    .bind(&id)
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("账户不存在".to_owned()));
    }
    Ok(Json(fetch_account(&state, &id).await?))
}

async fn delete_account(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    let id = common::id(&id, "账户 ID")?;
    let mut transaction = state.db.begin().await?;
    let dependency_count = sqlx::query_scalar::<_, i64>(
        r#"SELECT
             (SELECT COUNT(*) FROM transaction_legs WHERE account_id = ?) +
             (SELECT COUNT(*) FROM reconciliations WHERE account_id = ?)"#,
    )
    .bind(&id)
    .bind(&id)
    .fetch_one(&mut *transaction)
    .await?;

    if dependency_count > 0 {
        return Err(AppError::Conflict(
            "账户已有流水或对账记录，不能删除；请改为冻结账户".to_owned(),
        ));
    }

    let result = sqlx::query("DELETE FROM accounts WHERE id = ?")
        .bind(&id)
        .execute(&mut *transaction)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("账户不存在".to_owned()));
    }
    transaction.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

struct ValidatedAccount {
    name: String,
    institution: Option<String>,
    account_type: AccountType,
    base_currency: String,
    include_in_net_worth: bool,
}

fn validate(input: AccountInput) -> AppResult<ValidatedAccount> {
    Ok(ValidatedAccount {
        name: common::required_text(&input.name, "账户名称", 100)?,
        institution: common::optional_text(input.institution.as_deref(), "机构名称", 100)?,
        account_type: input.account_type,
        base_currency: common::major_currency(&input.base_currency, "基础币种")?,
        include_in_net_worth: input.include_in_net_worth,
    })
}

async fn fetch_account(state: &AppState, id: &str) -> AppResult<Account> {
    sqlx::query_as::<_, Account>("SELECT * FROM accounts WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("账户不存在".to_owned()))
}
