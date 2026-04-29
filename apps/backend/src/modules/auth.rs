use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use argon2::Argon2;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng};
use async_trait::async_trait;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::Row;
use sqlx::postgres::PgPool;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::modules::{ApiError, ApiState};

const SESSION_TTL_HOURS: i64 = 12;
const STATION_LOGIN_GRANT_TYPE: &str = "workstation";
const ALL_STATIONS_WILDCARD: &str = "*";

const PERMISSIONS: &[(&str, &str)] = &[
    ("auth.manage_users", "管理用户"),
    ("workflow.read", "查看流程"),
    ("workflow.write", "编辑流程"),
    ("workflow.run", "运行流程"),
    ("workflow.operate", "操作运行实例"),
    ("plugin.manage", "管理插件"),
    ("workstation.login", "登录工作站"),
    ("workstation.operate", "操作工作站"),
    ("pack.operate", "集包操作"),
];

const ROLES: &[(&str, &str, &[&str])] = &[
    (
        "SUPER_ADMIN",
        "超级管理员",
        &[
            "auth.manage_users",
            "workflow.read",
            "workflow.write",
            "workflow.run",
            "workflow.operate",
            "plugin.manage",
            "workstation.login",
            "workstation.operate",
            "pack.operate",
        ],
    ),
    (
        "ADMIN",
        "管理员",
        &[
            "auth.manage_users",
            "workflow.read",
            "workflow.write",
            "workflow.run",
            "workflow.operate",
            "plugin.manage",
        ],
    ),
    (
        "WORKFLOW_OPERATOR",
        "流程操作员",
        &["workflow.read", "workflow.run", "workflow.operate"],
    ),
    (
        "WORKSTATION_OPERATOR",
        "工作台操作员",
        &["workstation.login", "workstation.operate"],
    ),
    ("PACKER", "集包员", &["workstation.login", "pack.operate"]),
    ("VIEWER", "访客", &["workflow.read"]),
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthUserResponse {
    pub id: String,
    pub username: String,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub role: String,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
    pub is_active: bool,
    pub last_login_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthSessionResponse {
    pub id: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthPayload {
    pub access_token: String,
    pub expires_at: DateTime<Utc>,
    pub user: AuthUserResponse,
    pub session: AuthSessionResponse,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    #[serde(alias = "email", alias = "username")]
    pub login: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StationLoginRequest {
    #[serde(alias = "stationId")]
    pub station_id: String,
    #[serde(default, alias = "platformId")]
    pub platform_id: Option<String>,
    #[serde(alias = "username", alias = "email")]
    pub login: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StationAuthorizeRequest {
    #[serde(default)]
    pub required_permission: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StationAuthorizeResponse {
    pub user_id: String,
    pub station_id: String,
    pub platform_id: Option<String>,
    pub permissions: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateUserRequest {
    pub username: String,
    #[serde(default)]
    pub email: Option<String>,
    pub password: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub roles: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetUserActiveRequest {
    pub is_active: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssignRolesRequest {
    pub roles: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrantStationRequest {
    #[serde(alias = "stationId")]
    pub station_id: String,
    #[serde(default, alias = "platformId")]
    pub platform_id: Option<String>,
    #[serde(default)]
    pub grant_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user: AuthUserRecord,
    pub session: AuthSessionRecord,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AuthUserRecord {
    pub id: String,
    pub username: String,
    pub email: Option<String>,
    pub password_hash: String,
    pub display_name: Option<String>,
    pub is_active: bool,
    pub last_login_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct AuthSessionRecord {
    pub id: String,
    pub user_id: String,
    pub token_hash: String,
    pub station_id: Option<String>,
    pub platform_id: Option<String>,
    pub grant_type: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewSession {
    pub id: String,
    pub user_id: String,
    pub token_hash: String,
    pub station_id: Option<String>,
    pub platform_id: Option<String>,
    pub grant_type: Option<String>,
    pub user_agent: Option<String>,
    pub ip_address: Option<String>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewUser {
    pub username: String,
    pub email: Option<String>,
    pub password_hash: String,
    pub display_name: Option<String>,
}

#[async_trait]
pub trait AuthStore: Send + Sync {
    async fn initialize(&self) -> Result<(), String>;
    async fn find_user_by_login(&self, login: &str) -> Result<Option<AuthUserRecord>, String>;
    async fn get_user(&self, user_id: &str) -> Result<Option<AuthUserRecord>, String>;
    async fn create_user(&self, user: NewUser) -> Result<AuthUserRecord, String>;
    async fn set_user_active(&self, user_id: &str, is_active: bool) -> Result<AuthUserRecord, String>;
    async fn assign_roles(&self, user_id: &str, role_codes: &[String]) -> Result<(), String>;
    async fn grant_station(
        &self,
        user_id: &str,
        station_id: &str,
        platform_id: Option<&str>,
        grant_type: &str,
    ) -> Result<(), String>;
    async fn roles_and_permissions(&self, user_id: &str) -> Result<(Vec<String>, Vec<String>), String>;
    async fn has_station_grant(
        &self,
        user_id: &str,
        station_id: &str,
        platform_id: Option<&str>,
        grant_type: &str,
    ) -> Result<bool, String>;
    async fn create_session(&self, session: NewSession) -> Result<AuthSessionRecord, String>;
    async fn find_session_by_token_hash(&self, token_hash: &str) -> Result<Option<AuthSessionRecord>, String>;
    async fn touch_session(&self, session_id: &str) -> Result<(), String>;
    async fn revoke_session(&self, session_id: &str) -> Result<(), String>;
    async fn update_last_login(&self, user_id: &str) -> Result<(), String>;
}

pub struct PostgresAuthStore {
    pool: PgPool,
}

impl PostgresAuthStore {
    pub async fn new(pool: PgPool) -> Result<Self, String> {
        let store = Self { pool };
        store.initialize().await?;
        Ok(store)
    }

    async fn exec(&self, sql: &str) -> Result<(), String> {
        sqlx::query(sql)
            .execute(&self.pool)
            .await
            .map_err(|error| format!("auth schema update failed: {error}"))?;
        Ok(())
    }

    async fn seed_permission(&self, code: &str, name: &str) -> Result<(), String> {
        sqlx::query(
            r#"
            INSERT INTO auth_permissions (id, code, name)
            VALUES ($1, $2, $3)
            ON CONFLICT (code) DO UPDATE SET name = EXCLUDED.name
            "#,
        )
        .bind(format!("perm:{code}"))
        .bind(code)
        .bind(name)
        .execute(&self.pool)
        .await
        .map_err(|error| format!("failed to seed permission {code}: {error}"))?;
        Ok(())
    }

    async fn seed_role(&self, code: &str, name: &str, permissions: &[&str]) -> Result<(), String> {
        let role_id = format!("role:{code}");
        sqlx::query(
            r#"
            INSERT INTO auth_roles (id, code, name, is_system)
            VALUES ($1, $2, $3, TRUE)
            ON CONFLICT (code) DO UPDATE SET name = EXCLUDED.name, is_system = TRUE, updated_at = NOW()
            "#,
        )
        .bind(&role_id)
        .bind(code)
        .bind(name)
        .execute(&self.pool)
        .await
        .map_err(|error| format!("failed to seed role {code}: {error}"))?;

        for permission in permissions {
            sqlx::query(
                r#"
                INSERT INTO auth_role_permissions (role_id, permission_id)
                SELECT $1, id FROM auth_permissions WHERE code = $2
                ON CONFLICT DO NOTHING
                "#,
            )
            .bind(&role_id)
            .bind(permission)
            .execute(&self.pool)
            .await
            .map_err(|error| format!("failed to seed role permission {code}/{permission}: {error}"))?;
        }
        Ok(())
    }
}

#[async_trait]
impl AuthStore for PostgresAuthStore {
    async fn initialize(&self) -> Result<(), String> {
        self.exec(
            r#"
            CREATE TABLE IF NOT EXISTS auth_users (
              id TEXT PRIMARY KEY,
              username TEXT NOT NULL,
              email TEXT,
              password_hash TEXT NOT NULL,
              display_name TEXT,
              is_active BOOLEAN NOT NULL DEFAULT TRUE,
              password_changed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
              last_login_at TIMESTAMPTZ,
              created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
              updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#,
        )
        .await?;
        self.exec("CREATE UNIQUE INDEX IF NOT EXISTS idx_auth_users_username_lower ON auth_users (lower(username))")
            .await?;
        self.exec(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_auth_users_email_lower ON auth_users (lower(email)) WHERE email IS NOT NULL",
        )
        .await?;
        self.exec(
            r#"
            CREATE TABLE IF NOT EXISTS auth_sessions (
              id TEXT PRIMARY KEY,
              user_id TEXT NOT NULL REFERENCES auth_users(id) ON DELETE CASCADE,
              token_hash TEXT NOT NULL UNIQUE,
              user_agent TEXT,
              ip_address TEXT,
              expires_at TIMESTAMPTZ NOT NULL,
              revoked_at TIMESTAMPTZ,
              last_used_at TIMESTAMPTZ,
              created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#,
        )
        .await?;
        self.exec("ALTER TABLE auth_sessions ADD COLUMN IF NOT EXISTS station_id TEXT")
            .await?;
        self.exec("ALTER TABLE auth_sessions ADD COLUMN IF NOT EXISTS platform_id TEXT")
            .await?;
        self.exec("ALTER TABLE auth_sessions ADD COLUMN IF NOT EXISTS grant_type TEXT")
            .await?;
        self.exec("CREATE INDEX IF NOT EXISTS idx_auth_sessions_user_id ON auth_sessions(user_id)")
            .await?;
        self.exec("CREATE INDEX IF NOT EXISTS idx_auth_sessions_expires_at ON auth_sessions(expires_at)")
            .await?;
        self.exec(
            r#"
            CREATE TABLE IF NOT EXISTS auth_roles (
              id TEXT PRIMARY KEY,
              code TEXT NOT NULL UNIQUE,
              name TEXT NOT NULL,
              description TEXT,
              is_system BOOLEAN NOT NULL DEFAULT FALSE,
              created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
              updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#,
        )
        .await?;
        self.exec(
            r#"
            CREATE TABLE IF NOT EXISTS auth_permissions (
              id TEXT PRIMARY KEY,
              code TEXT NOT NULL UNIQUE,
              name TEXT NOT NULL,
              description TEXT,
              created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#,
        )
        .await?;
        self.exec(
            r#"
            CREATE TABLE IF NOT EXISTS auth_user_roles (
              user_id TEXT NOT NULL REFERENCES auth_users(id) ON DELETE CASCADE,
              role_id TEXT NOT NULL REFERENCES auth_roles(id) ON DELETE CASCADE,
              created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
              PRIMARY KEY (user_id, role_id)
            )
            "#,
        )
        .await?;
        self.exec(
            r#"
            CREATE TABLE IF NOT EXISTS auth_role_permissions (
              role_id TEXT NOT NULL REFERENCES auth_roles(id) ON DELETE CASCADE,
              permission_id TEXT NOT NULL REFERENCES auth_permissions(id) ON DELETE CASCADE,
              created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
              PRIMARY KEY (role_id, permission_id)
            )
            "#,
        )
        .await?;
        self.exec(
            r#"
            CREATE TABLE IF NOT EXISTS auth_station_grants (
              id TEXT PRIMARY KEY,
              user_id TEXT NOT NULL REFERENCES auth_users(id) ON DELETE CASCADE,
              station_id TEXT NOT NULL,
              platform_id TEXT,
              grant_type TEXT NOT NULL,
              is_active BOOLEAN NOT NULL DEFAULT TRUE,
              created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
              updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
              UNIQUE (user_id, station_id, platform_id, grant_type)
            )
            "#,
        )
        .await?;
        self.exec(
            "CREATE INDEX IF NOT EXISTS idx_auth_station_grants_lookup ON auth_station_grants(user_id, station_id, platform_id, grant_type)",
        )
        .await?;
        self.exec(
            r#"
            CREATE TABLE IF NOT EXISTS auth_audit_logs (
              id TEXT PRIMARY KEY,
              actor_user_id TEXT REFERENCES auth_users(id) ON DELETE SET NULL,
              target_user_id TEXT REFERENCES auth_users(id) ON DELETE SET NULL,
              action TEXT NOT NULL,
              resource_type TEXT NOT NULL,
              resource_id TEXT,
              detail JSONB NOT NULL DEFAULT '{}'::jsonb,
              created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#,
        )
        .await?;
        self.exec("CREATE INDEX IF NOT EXISTS idx_auth_audit_logs_actor ON auth_audit_logs(actor_user_id)")
            .await?;
        self.exec("CREATE INDEX IF NOT EXISTS idx_auth_audit_logs_created_at ON auth_audit_logs(created_at)")
            .await?;

        for (code, name) in PERMISSIONS {
            self.seed_permission(code, name).await?;
        }
        for (code, name, permissions) in ROLES {
            self.seed_role(code, name, permissions).await?;
        }
        Ok(())
    }

    async fn find_user_by_login(&self, login: &str) -> Result<Option<AuthUserRecord>, String> {
        let login = login.trim().to_lowercase();
        let row = sqlx::query(
            r#"
            SELECT id, username, email, password_hash, display_name, is_active, last_login_at, created_at, updated_at
            FROM auth_users
            WHERE lower(username) = $1 OR lower(email) = $1
            "#,
        )
        .bind(login)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| format!("failed to find auth user: {error}"))?;
        Ok(row.map(user_from_row))
    }

    async fn get_user(&self, user_id: &str) -> Result<Option<AuthUserRecord>, String> {
        let row = sqlx::query(
            r#"
            SELECT id, username, email, password_hash, display_name, is_active, last_login_at, created_at, updated_at
            FROM auth_users
            WHERE id = $1
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| format!("failed to get auth user: {error}"))?;
        Ok(row.map(user_from_row))
    }

    async fn create_user(&self, user: NewUser) -> Result<AuthUserRecord, String> {
        let id = Uuid::new_v4().to_string();
        let row = sqlx::query(
            r#"
            INSERT INTO auth_users (id, username, email, password_hash, display_name)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, username, email, password_hash, display_name, is_active, last_login_at, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(normalize_required(&user.username))
        .bind(user.email.as_deref().and_then(normalize_optional))
        .bind(user.password_hash)
        .bind(user.display_name.as_deref().and_then(normalize_optional))
        .fetch_one(&self.pool)
        .await
        .map_err(|error| format!("failed to create auth user: {error}"))?;
        Ok(user_from_row(row))
    }

    async fn set_user_active(&self, user_id: &str, is_active: bool) -> Result<AuthUserRecord, String> {
        let row = sqlx::query(
            r#"
            UPDATE auth_users
            SET is_active = $2, updated_at = NOW()
            WHERE id = $1
            RETURNING id, username, email, password_hash, display_name, is_active, last_login_at, created_at, updated_at
            "#,
        )
        .bind(user_id)
        .bind(is_active)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| format!("failed to update auth user: {error}"))?;
        Ok(user_from_row(row))
    }

    async fn assign_roles(&self, user_id: &str, role_codes: &[String]) -> Result<(), String> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| format!("failed to begin role assignment: {error}"))?;
        sqlx::query("DELETE FROM auth_user_roles WHERE user_id = $1")
            .bind(user_id)
            .execute(&mut *tx)
            .await
            .map_err(|error| format!("failed to clear auth user roles: {error}"))?;
        for code in role_codes {
            sqlx::query(
                r#"
                INSERT INTO auth_user_roles (user_id, role_id)
                SELECT $1, id FROM auth_roles WHERE code = $2
                ON CONFLICT DO NOTHING
                "#,
            )
            .bind(user_id)
            .bind(code.trim())
            .execute(&mut *tx)
            .await
            .map_err(|error| format!("failed to assign auth role: {error}"))?;
        }
        tx.commit()
            .await
            .map_err(|error| format!("failed to commit role assignment: {error}"))?;
        Ok(())
    }

    async fn grant_station(
        &self,
        user_id: &str,
        station_id: &str,
        platform_id: Option<&str>,
        grant_type: &str,
    ) -> Result<(), String> {
        sqlx::query(
            r#"
            INSERT INTO auth_station_grants (id, user_id, station_id, platform_id, grant_type)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (user_id, station_id, platform_id, grant_type)
            DO UPDATE SET is_active = TRUE, updated_at = NOW()
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(user_id)
        .bind(station_id)
        .bind(platform_id)
        .bind(grant_type)
        .execute(&self.pool)
        .await
        .map_err(|error| format!("failed to grant station: {error}"))?;
        Ok(())
    }

    async fn roles_and_permissions(&self, user_id: &str) -> Result<(Vec<String>, Vec<String>), String> {
        let role_rows = sqlx::query(
            r#"
            SELECT r.code
            FROM auth_user_roles ur
            JOIN auth_roles r ON r.id = ur.role_id
            WHERE ur.user_id = $1
            ORDER BY r.code
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| format!("failed to load auth roles: {error}"))?;
        let permission_rows = sqlx::query(
            r#"
            SELECT DISTINCT p.code
            FROM auth_user_roles ur
            JOIN auth_role_permissions rp ON rp.role_id = ur.role_id
            JOIN auth_permissions p ON p.id = rp.permission_id
            WHERE ur.user_id = $1
            ORDER BY p.code
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| format!("failed to load auth permissions: {error}"))?;
        Ok((
            role_rows.into_iter().map(|row| row.get::<String, _>("code")).collect(),
            permission_rows
                .into_iter()
                .map(|row| row.get::<String, _>("code"))
                .collect(),
        ))
    }

    async fn has_station_grant(
        &self,
        user_id: &str,
        station_id: &str,
        platform_id: Option<&str>,
        grant_type: &str,
    ) -> Result<bool, String> {
        let row = sqlx::query(
            r#"
            SELECT 1
            FROM auth_station_grants
            WHERE user_id = $1
              AND (station_id = $2 OR station_id = '*')
              AND grant_type = $4
              AND is_active = TRUE
              AND (platform_id IS NULL OR platform_id = $3)
            LIMIT 1
            "#,
        )
        .bind(user_id)
        .bind(station_id)
        .bind(platform_id)
        .bind(grant_type)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| format!("failed to check station grant: {error}"))?;
        Ok(row.is_some())
    }

    async fn create_session(&self, session: NewSession) -> Result<AuthSessionRecord, String> {
        let row = sqlx::query(
            r#"
            INSERT INTO auth_sessions (
              id, user_id, token_hash, station_id, platform_id, grant_type, user_agent, ip_address, expires_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id, user_id, token_hash, station_id, platform_id, grant_type, expires_at, revoked_at, last_used_at, created_at
            "#,
        )
        .bind(session.id)
        .bind(session.user_id)
        .bind(session.token_hash)
        .bind(session.station_id)
        .bind(session.platform_id)
        .bind(session.grant_type)
        .bind(session.user_agent)
        .bind(session.ip_address)
        .bind(session.expires_at)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| format!("failed to create auth session: {error}"))?;
        Ok(session_from_row(row))
    }

    async fn find_session_by_token_hash(&self, token_hash: &str) -> Result<Option<AuthSessionRecord>, String> {
        let row = sqlx::query(
            r#"
            SELECT id, user_id, token_hash, station_id, platform_id, grant_type, expires_at, revoked_at, last_used_at, created_at
            FROM auth_sessions
            WHERE token_hash = $1
            "#,
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| format!("failed to find auth session: {error}"))?;
        Ok(row.map(session_from_row))
    }

    async fn touch_session(&self, session_id: &str) -> Result<(), String> {
        sqlx::query("UPDATE auth_sessions SET last_used_at = NOW() WHERE id = $1")
            .bind(session_id)
            .execute(&self.pool)
            .await
            .map_err(|error| format!("failed to touch auth session: {error}"))?;
        Ok(())
    }

    async fn revoke_session(&self, session_id: &str) -> Result<(), String> {
        sqlx::query("UPDATE auth_sessions SET revoked_at = NOW() WHERE id = $1 AND revoked_at IS NULL")
            .bind(session_id)
            .execute(&self.pool)
            .await
            .map_err(|error| format!("failed to revoke auth session: {error}"))?;
        Ok(())
    }

    async fn update_last_login(&self, user_id: &str) -> Result<(), String> {
        sqlx::query("UPDATE auth_users SET last_login_at = NOW(), updated_at = NOW() WHERE id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map_err(|error| format!("failed to update last login: {error}"))?;
        Ok(())
    }
}

#[derive(Default)]
pub struct InMemoryAuthStore {
    state: RwLock<InMemoryAuthState>,
}

#[derive(Default)]
struct InMemoryAuthState {
    users: HashMap<String, AuthUserRecord>,
    roles: HashMap<String, Vec<String>>,
    sessions: HashMap<String, AuthSessionRecord>,
    station_grants: HashSet<(String, String, Option<String>, String)>,
}

#[async_trait]
impl AuthStore for InMemoryAuthStore {
    async fn initialize(&self) -> Result<(), String> {
        Ok(())
    }

    async fn find_user_by_login(&self, login: &str) -> Result<Option<AuthUserRecord>, String> {
        let login = login.trim().to_lowercase();
        let state = self.state.read().await;
        Ok(state
            .users
            .values()
            .find(|user| {
                user.username.to_lowercase() == login
                    || user.email.as_deref().map(str::to_lowercase).as_deref() == Some(login.as_str())
            })
            .cloned())
    }

    async fn get_user(&self, user_id: &str) -> Result<Option<AuthUserRecord>, String> {
        Ok(self.state.read().await.users.get(user_id).cloned())
    }

    async fn create_user(&self, user: NewUser) -> Result<AuthUserRecord, String> {
        let mut state = self.state.write().await;
        let now = Utc::now();
        let record = AuthUserRecord {
            id: Uuid::new_v4().to_string(),
            username: normalize_required(&user.username),
            email: user.email.as_deref().and_then(normalize_optional),
            password_hash: user.password_hash,
            display_name: user.display_name.as_deref().and_then(normalize_optional),
            is_active: true,
            last_login_at: None,
            created_at: now,
            updated_at: now,
        };
        state.users.insert(record.id.clone(), record.clone());
        Ok(record)
    }

    async fn set_user_active(&self, user_id: &str, is_active: bool) -> Result<AuthUserRecord, String> {
        let mut state = self.state.write().await;
        let user = state
            .users
            .get_mut(user_id)
            .ok_or_else(|| "auth user not found".to_string())?;
        user.is_active = is_active;
        user.updated_at = Utc::now();
        Ok(user.clone())
    }

    async fn assign_roles(&self, user_id: &str, role_codes: &[String]) -> Result<(), String> {
        self.state.write().await.roles.insert(
            user_id.to_string(),
            role_codes.iter().map(|role| role.trim().to_string()).collect(),
        );
        Ok(())
    }

    async fn grant_station(
        &self,
        user_id: &str,
        station_id: &str,
        platform_id: Option<&str>,
        grant_type: &str,
    ) -> Result<(), String> {
        self.state.write().await.station_grants.insert((
            user_id.to_string(),
            station_id.to_string(),
            platform_id.map(str::to_string),
            grant_type.to_string(),
        ));
        Ok(())
    }

    async fn roles_and_permissions(&self, user_id: &str) -> Result<(Vec<String>, Vec<String>), String> {
        let state = self.state.read().await;
        let roles = state.roles.get(user_id).cloned().unwrap_or_default();
        Ok((roles.clone(), permissions_for_roles(&roles)))
    }

    async fn has_station_grant(
        &self,
        user_id: &str,
        station_id: &str,
        platform_id: Option<&str>,
        grant_type: &str,
    ) -> Result<bool, String> {
        let state = self.state.read().await;
        let user_id = user_id.to_string();
        let station_id = station_id.to_string();
        let platform_id = platform_id.map(str::to_string);
        let grant_type = grant_type.to_string();
        Ok(state.station_grants.contains(&(
            user_id.clone(),
            station_id.clone(),
            platform_id.clone(),
            grant_type.clone(),
        )) || state
            .station_grants
            .contains(&(user_id.clone(), station_id, None, grant_type.clone()))
            || state
                .station_grants
                .contains(&(user_id.clone(), "*".to_string(), platform_id, grant_type.clone()))
            || state
                .station_grants
                .contains(&(user_id, "*".to_string(), None, grant_type)))
    }

    async fn create_session(&self, session: NewSession) -> Result<AuthSessionRecord, String> {
        let record = AuthSessionRecord {
            id: session.id,
            user_id: session.user_id,
            token_hash: session.token_hash,
            station_id: session.station_id,
            platform_id: session.platform_id,
            grant_type: session.grant_type,
            expires_at: session.expires_at,
            revoked_at: None,
            last_used_at: None,
            created_at: Utc::now(),
        };
        self.state
            .write()
            .await
            .sessions
            .insert(record.token_hash.clone(), record.clone());
        Ok(record)
    }

    async fn find_session_by_token_hash(&self, token_hash: &str) -> Result<Option<AuthSessionRecord>, String> {
        Ok(self.state.read().await.sessions.get(token_hash).cloned())
    }

    async fn touch_session(&self, session_id: &str) -> Result<(), String> {
        for session in self.state.write().await.sessions.values_mut() {
            if session.id == session_id {
                session.last_used_at = Some(Utc::now());
            }
        }
        Ok(())
    }

    async fn revoke_session(&self, session_id: &str) -> Result<(), String> {
        for session in self.state.write().await.sessions.values_mut() {
            if session.id == session_id {
                session.revoked_at = Some(Utc::now());
            }
        }
        Ok(())
    }

    async fn update_last_login(&self, user_id: &str) -> Result<(), String> {
        if let Some(user) = self.state.write().await.users.get_mut(user_id) {
            user.last_login_at = Some(Utc::now());
            user.updated_at = Utc::now();
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct AuthService {
    store: Arc<dyn AuthStore>,
}

impl AuthService {
    pub fn new(store: Arc<dyn AuthStore>) -> Self {
        Self { store }
    }

    pub async fn bootstrap_from_env(&self) -> Result<(), String> {
        let username = std::env::var("SES_AUTH_BOOTSTRAP_USERNAME")
            .ok()
            .and_then(|value| normalize_optional(&value));
        let password = std::env::var("SES_AUTH_BOOTSTRAP_PASSWORD")
            .ok()
            .and_then(|value| normalize_optional(&value));
        let Some(username) = username else {
            return Ok(());
        };
        let Some(password) = password else {
            return Ok(());
        };
        self.bootstrap_super_admin(
            &username,
            std::env::var("SES_AUTH_BOOTSTRAP_EMAIL")
                .ok()
                .and_then(|value| normalize_optional(&value))
                .as_deref(),
            &password,
        )
        .await
        .map(|_| ())
    }

    pub async fn bootstrap_super_admin(
        &self,
        username: &str,
        email: Option<&str>,
        password: &str,
    ) -> Result<AuthUserResponse, String> {
        if self.store.find_user_by_login(&username).await?.is_some() {
            let user = self
                .store
                .find_user_by_login(username)
                .await?
                .expect("bootstrap user should exist");
            self.store.assign_roles(&user.id, &["SUPER_ADMIN".to_string()]).await?;
            return self
                .user_response(user)
                .await
                .map_err(|error| format!("failed to load bootstrap user: {error:?}"));
        }
        let user = self
            .store
            .create_user(NewUser {
                username: username.to_string(),
                email: email.and_then(normalize_optional),
                password_hash: hash_password(&password)?,
                display_name: Some("SES Super Admin".to_string()),
            })
            .await?;
        self.store.assign_roles(&user.id, &["SUPER_ADMIN".to_string()]).await?;
        self.store
            .grant_station(&user.id, ALL_STATIONS_WILDCARD, None, STATION_LOGIN_GRANT_TYPE)
            .await?;
        self.user_response(user)
            .await
            .map_err(|error| format!("failed to load bootstrap user: {error:?}"))
    }

    pub async fn login(&self, request: LoginRequest, headers: &HeaderMap) -> Result<AuthPayload, ApiError> {
        self.login_inner(&request.login, &request.password, headers, None).await
    }

    pub async fn station_login(
        &self,
        request: StationLoginRequest,
        headers: &HeaderMap,
    ) -> Result<AuthPayload, ApiError> {
        let station_id = normalize_optional(&request.station_id)
            .ok_or_else(|| ApiError::BadRequest("stationId is required".to_string()))?;
        let platform_id = request.platform_id.as_deref().and_then(normalize_optional);
        self.login_inner(
            &request.login,
            &request.password,
            headers,
            Some(SessionStationScope {
                station_id,
                platform_id,
                grant_type: STATION_LOGIN_GRANT_TYPE.to_string(),
            }),
        )
        .await
    }

    pub async fn authenticate(&self, headers: &HeaderMap) -> Result<AuthContext, ApiError> {
        let token = bearer_token(headers).ok_or_else(|| ApiError::Unauthorized("missing bearer token".to_string()))?;
        let token_hash = token_hash(&token);
        let session = self
            .store
            .find_session_by_token_hash(&token_hash)
            .await
            .map_err(ApiError::ServiceUnavailable)?
            .ok_or_else(|| ApiError::Unauthorized("invalid bearer token".to_string()))?;
        if session.revoked_at.is_some() || session.expires_at <= Utc::now() {
            return Err(ApiError::Unauthorized("expired or revoked session".to_string()));
        }
        let user = self
            .store
            .get_user(&session.user_id)
            .await
            .map_err(ApiError::ServiceUnavailable)?
            .ok_or_else(|| ApiError::Unauthorized("session user not found".to_string()))?;
        if !user.is_active {
            return Err(ApiError::Unauthorized("user is disabled".to_string()));
        }
        let (roles, permissions) = self
            .store
            .roles_and_permissions(&user.id)
            .await
            .map_err(ApiError::ServiceUnavailable)?;
        self.store
            .touch_session(&session.id)
            .await
            .map_err(ApiError::ServiceUnavailable)?;
        Ok(AuthContext {
            user,
            session,
            roles,
            permissions,
        })
    }

    pub async fn logout(&self, headers: &HeaderMap) -> Result<(), ApiError> {
        let context = self.authenticate(headers).await?;
        self.store
            .revoke_session(&context.session.id)
            .await
            .map_err(ApiError::ServiceUnavailable)
    }

    pub async fn require_permission(&self, headers: &HeaderMap, permission: &str) -> Result<AuthContext, ApiError> {
        let context = self.authenticate(headers).await?;
        if context.permissions.iter().any(|candidate| candidate == permission) {
            Ok(context)
        } else {
            Err(ApiError::Forbidden(format!("missing permission: {permission}")))
        }
    }

    pub async fn station_authorize(
        &self,
        headers: &HeaderMap,
        request: StationAuthorizeRequest,
    ) -> Result<StationAuthorizeResponse, ApiError> {
        let required_permission = request
            .required_permission
            .as_deref()
            .and_then(normalize_optional)
            .unwrap_or_else(|| "workstation.operate".to_string());
        let context = self.require_permission(headers, &required_permission).await?;
        let station_id = context
            .session
            .station_id
            .clone()
            .ok_or_else(|| ApiError::Forbidden("session is not bound to a station".to_string()))?;
        let grant_type = context
            .session
            .grant_type
            .clone()
            .unwrap_or_else(|| STATION_LOGIN_GRANT_TYPE.to_string());
        let allowed = self
            .store
            .has_station_grant(
                &context.user.id,
                &station_id,
                context.session.platform_id.as_deref(),
                &grant_type,
            )
            .await
            .map_err(ApiError::ServiceUnavailable)?;
        if !allowed {
            return Err(ApiError::Forbidden("station is not permitted".to_string()));
        }
        Ok(StationAuthorizeResponse {
            user_id: context.user.id,
            station_id,
            platform_id: context.session.platform_id,
            permissions: context.permissions,
        })
    }

    pub async fn create_user(
        &self,
        headers: &HeaderMap,
        request: CreateUserRequest,
    ) -> Result<AuthUserResponse, ApiError> {
        self.require_permission(headers, "auth.manage_users").await?;
        let username = normalize_optional(&request.username)
            .ok_or_else(|| ApiError::BadRequest("username is required".to_string()))?;
        if request.password.trim().len() < 6 {
            return Err(ApiError::BadRequest(
                "password must be at least 6 characters".to_string(),
            ));
        }
        let user = self
            .store
            .create_user(NewUser {
                username,
                email: request.email.as_deref().and_then(normalize_optional),
                password_hash: hash_password(&request.password).map_err(ApiError::BadRequest)?,
                display_name: request.display_name.as_deref().and_then(normalize_optional),
            })
            .await
            .map_err(ApiError::BadRequest)?;
        if !request.roles.is_empty() {
            self.store
                .assign_roles(&user.id, &request.roles)
                .await
                .map_err(ApiError::ServiceUnavailable)?;
        }
        self.store
            .grant_station(&user.id, ALL_STATIONS_WILDCARD, None, STATION_LOGIN_GRANT_TYPE)
            .await
            .map_err(ApiError::ServiceUnavailable)?;
        self.user_response(user).await
    }

    pub async fn set_user_active(
        &self,
        headers: &HeaderMap,
        user_id: &str,
        request: SetUserActiveRequest,
    ) -> Result<AuthUserResponse, ApiError> {
        self.require_permission(headers, "auth.manage_users").await?;
        let user = self
            .store
            .set_user_active(user_id, request.is_active)
            .await
            .map_err(ApiError::BadRequest)?;
        self.user_response(user).await
    }

    pub async fn assign_roles(
        &self,
        headers: &HeaderMap,
        user_id: &str,
        request: AssignRolesRequest,
    ) -> Result<AuthUserResponse, ApiError> {
        self.require_permission(headers, "auth.manage_users").await?;
        self.store
            .assign_roles(user_id, &request.roles)
            .await
            .map_err(ApiError::ServiceUnavailable)?;
        let user = self
            .store
            .get_user(user_id)
            .await
            .map_err(ApiError::ServiceUnavailable)?
            .ok_or_else(|| ApiError::NotFound("auth user not found".to_string()))?;
        self.user_response(user).await
    }

    pub async fn grant_station(
        &self,
        headers: &HeaderMap,
        user_id: &str,
        request: GrantStationRequest,
    ) -> Result<Value, ApiError> {
        self.require_permission(headers, "auth.manage_users").await?;
        let station_id = normalize_optional(&request.station_id)
            .ok_or_else(|| ApiError::BadRequest("stationId is required".to_string()))?;
        let grant_type = request
            .grant_type
            .as_deref()
            .and_then(normalize_optional)
            .unwrap_or_else(|| STATION_LOGIN_GRANT_TYPE.to_string());
        self.store
            .grant_station(
                user_id,
                &station_id,
                request.platform_id.as_deref().and_then(normalize_optional).as_deref(),
                &grant_type,
            )
            .await
            .map_err(ApiError::ServiceUnavailable)?;
        Ok(json!({
            "userId": user_id,
            "stationId": station_id,
            "platformId": request.platform_id,
            "grantType": grant_type
        }))
    }

    async fn login_inner(
        &self,
        login: &str,
        password: &str,
        headers: &HeaderMap,
        station_scope: Option<SessionStationScope>,
    ) -> Result<AuthPayload, ApiError> {
        let login = normalize_optional(login).ok_or_else(|| ApiError::BadRequest("login is required".to_string()))?;
        let user = self
            .store
            .find_user_by_login(&login)
            .await
            .map_err(ApiError::ServiceUnavailable)?
            .ok_or_else(|| ApiError::Unauthorized("invalid username or password".to_string()))?;
        if !user.is_active || !verify_password(password, &user.password_hash) {
            return Err(ApiError::Unauthorized("invalid username or password".to_string()));
        }
        let (roles, permissions) = self
            .store
            .roles_and_permissions(&user.id)
            .await
            .map_err(ApiError::ServiceUnavailable)?;
        if let Some(scope) = station_scope.as_ref() {
            if !permissions.iter().any(|permission| permission == "workstation.login") {
                return Err(ApiError::Forbidden("missing permission: workstation.login".to_string()));
            }
            let allowed = self
                .store
                .has_station_grant(
                    &user.id,
                    &scope.station_id,
                    scope.platform_id.as_deref(),
                    &scope.grant_type,
                )
                .await
                .map_err(ApiError::ServiceUnavailable)?;
            if !allowed {
                return Err(ApiError::Forbidden("station is not permitted".to_string()));
            }
        }

        let access_token = new_access_token();
        let expires_at = Utc::now() + Duration::hours(SESSION_TTL_HOURS);
        let session = self
            .store
            .create_session(NewSession {
                id: Uuid::new_v4().to_string(),
                user_id: user.id.clone(),
                token_hash: token_hash(&access_token),
                station_id: station_scope.as_ref().map(|scope| scope.station_id.clone()),
                platform_id: station_scope.as_ref().and_then(|scope| scope.platform_id.clone()),
                grant_type: station_scope.as_ref().map(|scope| scope.grant_type.clone()),
                user_agent: header_text(headers, axum::http::header::USER_AGENT.as_str()),
                ip_address: header_text(headers, "x-forwarded-for"),
                expires_at,
            })
            .await
            .map_err(ApiError::ServiceUnavailable)?;
        self.store
            .update_last_login(&user.id)
            .await
            .map_err(ApiError::ServiceUnavailable)?;
        Ok(AuthPayload {
            access_token,
            expires_at,
            user: response_from_parts(user, roles, permissions),
            session: session_response(session),
        })
    }

    async fn user_response(&self, user: AuthUserRecord) -> Result<AuthUserResponse, ApiError> {
        let (roles, permissions) = self
            .store
            .roles_and_permissions(&user.id)
            .await
            .map_err(ApiError::ServiceUnavailable)?;
        Ok(response_from_parts(user, roles, permissions))
    }
}

#[derive(Debug)]
struct SessionStationScope {
    station_id: String,
    platform_id: Option<String>,
    grant_type: String,
}

pub fn router() -> Router<ApiState> {
    Router::new()
        .route("/login", post(login))
        .route("/register", post(disabled_register))
        .route("/me", get(me))
        .route("/logout", post(logout))
        .route("/station-login", post(station_login))
        .route("/station-authorize", post(station_authorize))
        .route("/admin/users", post(create_user))
        .route("/admin/users/{user_id}/active", post(set_user_active))
        .route("/admin/users/{user_id}/roles", post(assign_roles))
        .route("/admin/users/{user_id}/station-grants", post(grant_station))
}

async fn disabled_register() -> Result<StatusCode, ApiError> {
    Err(ApiError::Forbidden(
        "public registration is disabled; ask an administrator to create an account".to_string(),
    ))
}

async fn login(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<LoginRequest>,
) -> Result<Json<AuthPayload>, ApiError> {
    Ok(Json(state.auth.login(request, &headers).await?))
}

async fn station_login(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<StationLoginRequest>,
) -> Result<Json<AuthPayload>, ApiError> {
    Ok(Json(state.auth.station_login(request, &headers).await?))
}

async fn station_authorize(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<StationAuthorizeRequest>,
) -> Result<Json<StationAuthorizeResponse>, ApiError> {
    Ok(Json(state.auth.station_authorize(&headers, request).await?))
}

async fn me(State(state): State<ApiState>, headers: HeaderMap) -> Result<Json<serde_json::Value>, ApiError> {
    let context = state.auth.authenticate(&headers).await?;
    Ok(Json(json!({
        "user": response_from_parts(context.user, context.roles, context.permissions),
        "session": session_response(context.session)
    })))
}

async fn logout(State(state): State<ApiState>, headers: HeaderMap) -> Result<StatusCode, ApiError> {
    state.auth.logout(&headers).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn create_user(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<CreateUserRequest>,
) -> Result<Json<AuthUserResponse>, ApiError> {
    Ok(Json(state.auth.create_user(&headers, request).await?))
}

async fn set_user_active(
    State(state): State<ApiState>,
    headers: HeaderMap,
    axum::extract::Path(user_id): axum::extract::Path<String>,
    Json(request): Json<SetUserActiveRequest>,
) -> Result<Json<AuthUserResponse>, ApiError> {
    Ok(Json(state.auth.set_user_active(&headers, &user_id, request).await?))
}

async fn assign_roles(
    State(state): State<ApiState>,
    headers: HeaderMap,
    axum::extract::Path(user_id): axum::extract::Path<String>,
    Json(request): Json<AssignRolesRequest>,
) -> Result<Json<AuthUserResponse>, ApiError> {
    Ok(Json(state.auth.assign_roles(&headers, &user_id, request).await?))
}

async fn grant_station(
    State(state): State<ApiState>,
    headers: HeaderMap,
    axum::extract::Path(user_id): axum::extract::Path<String>,
    Json(request): Json<GrantStationRequest>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(state.auth.grant_station(&headers, &user_id, request).await?))
}

fn response_from_parts(user: AuthUserRecord, roles: Vec<String>, permissions: Vec<String>) -> AuthUserResponse {
    let role = roles.first().cloned().unwrap_or_else(|| "VIEWER".to_string());
    AuthUserResponse {
        id: user.id,
        username: user.username,
        email: user.email,
        display_name: user.display_name,
        role,
        roles,
        permissions,
        is_active: user.is_active,
        last_login_at: user.last_login_at,
        created_at: user.created_at,
        updated_at: user.updated_at,
    }
}

fn session_response(session: AuthSessionRecord) -> AuthSessionResponse {
    AuthSessionResponse {
        id: session.id,
        expires_at: session.expires_at,
        created_at: session.created_at,
        last_used_at: session.last_used_at,
    }
}

fn user_from_row(row: sqlx::postgres::PgRow) -> AuthUserRecord {
    AuthUserRecord {
        id: row.get("id"),
        username: row.get("username"),
        email: row.get("email"),
        password_hash: row.get("password_hash"),
        display_name: row.get("display_name"),
        is_active: row.get("is_active"),
        last_login_at: row.get("last_login_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn session_from_row(row: sqlx::postgres::PgRow) -> AuthSessionRecord {
    AuthSessionRecord {
        id: row.get("id"),
        user_id: row.get("user_id"),
        token_hash: row.get("token_hash"),
        station_id: row.get("station_id"),
        platform_id: row.get("platform_id"),
        grant_type: row.get("grant_type"),
        expires_at: row.get("expires_at"),
        revoked_at: row.get("revoked_at"),
        last_used_at: row.get("last_used_at"),
        created_at: row.get("created_at"),
    }
}

fn permissions_for_roles(roles: &[String]) -> Vec<String> {
    let mut permissions = HashSet::new();
    for role in roles {
        if let Some((_, _, role_permissions)) = ROLES.iter().find(|(code, _, _)| code == role) {
            permissions.extend(role_permissions.iter().map(|permission| (*permission).to_string()));
        }
    }
    let mut permissions = permissions.into_iter().collect::<Vec<_>>();
    permissions.sort();
    permissions
}

fn normalize_required(value: &str) -> String {
    value.trim().to_string()
}

fn normalize_optional(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|error| format!("failed to hash password: {error}"))
}

fn verify_password(password: &str, password_hash: &str) -> bool {
    PasswordHash::new(password_hash)
        .ok()
        .is_some_and(|parsed| Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok())
}

fn new_access_token() -> String {
    format!("ses_{}_{}", Uuid::new_v4(), Uuid::new_v4())
}

fn token_hash(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

fn bearer_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn header_text(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .and_then(normalize_optional)
}
