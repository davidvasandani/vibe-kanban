use sqlx::{FromRow, SqlitePool};

#[derive(Clone, Debug, FromRow)]
pub struct McpGatewayConnection {
    pub id: String,
    pub owner_user_id: String,
    pub machine_id: String,
    pub server_name: String,
    pub upstream_url: String,
    pub transport: String,
    pub auth_kind: String,
    pub gateway_token_hash: Vec<u8>,
    pub encrypted_credentials: Option<String>,
    pub credential_version: i64,
    pub status: String,
    pub expires_at: Option<String>,
    pub last_error_code: Option<String>,
}

impl McpGatewayConnection {
    pub async fn find_bound(
        pool: &SqlitePool,
        id: &str,
        user_id: &str,
        machine_id: &str,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(
            r#"SELECT id, owner_user_id, machine_id, server_name, upstream_url,
                      transport, auth_kind, gateway_token_hash, encrypted_credentials,
                      credential_version, status, expires_at, last_error_code
               FROM mcp_gateway_connections
               WHERE id = ? AND owner_user_id = ? AND machine_id = ?"#,
        )
        .bind(id)
        .bind(user_id)
        .bind(machine_id)
        .fetch_optional(pool)
        .await
    }

    pub async fn disconnect(pool: &SqlitePool, id: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            r#"UPDATE mcp_gateway_connections
               SET encrypted_credentials = NULL, status = 'disconnected',
                   gateway_token_hash = randomblob(32), disconnected_at = datetime('now', 'subsec'),
                   updated_at = datetime('now', 'subsec')
               WHERE id = ?"#,
        )
        .bind(id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }
}
