use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use sqlx::{Row, postgres::PgPool};
use tokio::sync::RwLock;

const PLUGIN_AUTO_REGISTER_BASE_URLS_KEY: &str = "plugin_auto_register_base_urls";

#[async_trait]
pub trait SystemSettingsStore: Send + Sync {
    async fn load_plugin_auto_register_base_urls(&self) -> Result<Vec<String>, String>;
    async fn save_plugin_auto_register_base_urls(&self, base_urls: &[String]) -> Result<(), String>;
}

#[derive(Default)]
pub struct InMemorySystemSettingsStore {
    plugin_auto_register_base_urls: Arc<RwLock<Vec<String>>>,
}

impl InMemorySystemSettingsStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl SystemSettingsStore for InMemorySystemSettingsStore {
    async fn load_plugin_auto_register_base_urls(&self) -> Result<Vec<String>, String> {
        Ok(self.plugin_auto_register_base_urls.read().await.clone())
    }

    async fn save_plugin_auto_register_base_urls(&self, base_urls: &[String]) -> Result<(), String> {
        *self.plugin_auto_register_base_urls.write().await = base_urls.to_vec();
        Ok(())
    }
}

pub struct PostgresSystemSettingsStore {
    pool: PgPool,
}

impl PostgresSystemSettingsStore {
    pub async fn new(pool: PgPool) -> Result<Self, String> {
        let store = Self { pool };
        store.init_schema().await?;
        Ok(store)
    }

    async fn init_schema(&self) -> Result<(), String> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS system_settings (
                key TEXT PRIMARY KEY,
                value JSONB NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|error| format!("Failed to create system_settings table: {error}"))?;
        Ok(())
    }
}

#[async_trait]
impl SystemSettingsStore for PostgresSystemSettingsStore {
    async fn load_plugin_auto_register_base_urls(&self) -> Result<Vec<String>, String> {
        let row = sqlx::query(
            r#"
            SELECT value
            FROM system_settings
            WHERE key = $1
            "#,
        )
        .bind(PLUGIN_AUTO_REGISTER_BASE_URLS_KEY)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| format!("Failed to load plugin auto registration settings: {error}"))?;

        let Some(row) = row else {
            return Ok(Vec::new());
        };

        let value = row
            .try_get::<Value, _>("value")
            .map_err(|error| format!("Failed to read plugin auto registration settings: {error}"))?;

        serde_json::from_value::<Vec<String>>(value)
            .map_err(|error| format!("Failed to parse plugin auto registration settings: {error}"))
    }

    async fn save_plugin_auto_register_base_urls(&self, base_urls: &[String]) -> Result<(), String> {
        let value = serde_json::to_value(base_urls)
            .map_err(|error| format!("Failed to serialize plugin auto registration settings: {error}"))?;

        sqlx::query(
            r#"
            INSERT INTO system_settings (key, value)
            VALUES ($1, $2)
            ON CONFLICT (key)
            DO UPDATE SET value = EXCLUDED.value, updated_at = NOW()
            "#,
        )
        .bind(PLUGIN_AUTO_REGISTER_BASE_URLS_KEY)
        .bind(value)
        .execute(&self.pool)
        .await
        .map_err(|error| format!("Failed to save plugin auto registration settings: {error}"))?;

        Ok(())
    }
}
