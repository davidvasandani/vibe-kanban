use api_types::OrganizationEnvVar;
use sqlx::PgPool;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum OrganizationEnvVarError {
    #[error("env var name already exists for this organization")]
    NameConflict,
    #[error("env var not found")]
    NotFound,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

pub struct OrganizationEnvVarRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> OrganizationEnvVarRepository<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn list(
        &self,
        organization_id: Uuid,
    ) -> Result<Vec<OrganizationEnvVar>, OrganizationEnvVarError> {
        let rows = sqlx::query_as!(
            OrganizationEnvVar,
            r#"
            SELECT
                id              AS "id!: Uuid",
                organization_id AS "organization_id!: Uuid",
                name            AS "name!",
                created_at      AS "created_at!",
                updated_at      AS "updated_at!"
            FROM organization_env_vars
            WHERE organization_id = $1
            ORDER BY name
            "#,
            organization_id
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows)
    }

    /// Fetch all env vars for an organization as `(name, encrypted_value)`
    /// pairs. Unlike `list`, this includes the encrypted payload so callers can
    /// decrypt it — used only to resolve env vars for injection into agents.
    pub async fn list_with_encrypted_values(
        &self,
        organization_id: Uuid,
    ) -> Result<Vec<(String, String)>, OrganizationEnvVarError> {
        let rows = sqlx::query!(
            r#"
            SELECT name AS "name!", encrypted_value AS "encrypted_value!"
            FROM organization_env_vars
            WHERE organization_id = $1
            ORDER BY name
            "#,
            organization_id
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| (row.name, row.encrypted_value))
            .collect())
    }

    pub async fn create(
        &self,
        organization_id: Uuid,
        name: &str,
        encrypted_value: &str,
    ) -> Result<OrganizationEnvVar, OrganizationEnvVarError> {
        let row = sqlx::query_as!(
            OrganizationEnvVar,
            r#"
            INSERT INTO organization_env_vars (organization_id, name, encrypted_value)
            VALUES ($1, $2, $3)
            RETURNING
                id              AS "id!: Uuid",
                organization_id AS "organization_id!: Uuid",
                name            AS "name!",
                created_at      AS "created_at!",
                updated_at      AS "updated_at!"
            "#,
            organization_id,
            name,
            encrypted_value
        )
        .fetch_one(self.pool)
        .await
        .map_err(|e| {
            if let Some(db_err) = e.as_database_error()
                && db_err.is_unique_violation()
            {
                return OrganizationEnvVarError::NameConflict;
            }
            OrganizationEnvVarError::from(e)
        })?;

        Ok(row)
    }

    pub async fn update_value(
        &self,
        organization_id: Uuid,
        id: Uuid,
        encrypted_value: &str,
    ) -> Result<OrganizationEnvVar, OrganizationEnvVarError> {
        let row = sqlx::query_as!(
            OrganizationEnvVar,
            r#"
            UPDATE organization_env_vars
            SET encrypted_value = $3, updated_at = now()
            WHERE id = $1 AND organization_id = $2
            RETURNING
                id              AS "id!: Uuid",
                organization_id AS "organization_id!: Uuid",
                name            AS "name!",
                created_at      AS "created_at!",
                updated_at      AS "updated_at!"
            "#,
            id,
            organization_id,
            encrypted_value
        )
        .fetch_optional(self.pool)
        .await?
        .ok_or(OrganizationEnvVarError::NotFound)?;

        Ok(row)
    }

    pub async fn delete(
        &self,
        organization_id: Uuid,
        id: Uuid,
    ) -> Result<(), OrganizationEnvVarError> {
        let result = sqlx::query!(
            r#"
            DELETE FROM organization_env_vars
            WHERE id = $1 AND organization_id = $2
            "#,
            id,
            organization_id
        )
        .execute(self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(OrganizationEnvVarError::NotFound);
        }

        Ok(())
    }
}
