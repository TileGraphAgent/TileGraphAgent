/// HTTP client for Neo4j Bolt HTTP API (V1 uses Cypher HTTP endpoint).
/// Production would use bolt:// driver. V1 uses HTTP for portability.

use serde::{Deserialize, Serialize};
use tilegraph_core::Result;

#[derive(Debug, Clone)]
pub struct Neo4jConfig {
    pub url: String,          // e.g. http://localhost:7474
    pub username: String,
    pub password: String,
    pub database: String,     // e.g. "neo4j"
}

impl Neo4jConfig {
    pub fn from_env() -> Self {
        Self {
            url: std::env::var("NEO4J_URL").unwrap_or_else(|_| "http://localhost:7474".to_string()),
            username: std::env::var("NEO4J_USER").unwrap_or_else(|_| "neo4j".to_string()),
            password: std::env::var("NEO4J_PASSWORD").unwrap_or_else(|_| "password".to_string()),
            database: std::env::var("NEO4J_DATABASE").unwrap_or_else(|_| "neo4j".to_string()),
        }
    }
}

#[derive(Serialize)]
struct CypherRequest {
    statements: Vec<Statement>,
}

#[derive(Serialize)]
struct Statement {
    statement: String,
}

#[derive(Deserialize, Debug)]
pub struct CypherResponse {
    pub results: Vec<serde_json::Value>,
    pub errors: Vec<serde_json::Value>,
}

pub struct Neo4jClient {
    config: Neo4jConfig,
    http: reqwest::Client,
}

impl Neo4jClient {
    pub fn new(config: Neo4jConfig) -> Self {
        Self {
            config,
            http: reqwest::Client::new(),
        }
    }

    pub async fn execute(&self, cypher: &str) -> Result<CypherResponse> {
        let url = format!(
            "{}/db/{}/tx/commit",
            self.config.url, self.config.database
        );
        let body = CypherRequest {
            statements: vec![Statement { statement: cypher.to_string() }],
        };

        let resp = self.http
            .post(&url)
            .basic_auth(&self.config.username, Some(&self.config.password))
            .json(&body)
            .send()
            .await
            .map_err(|e| tilegraph_core::TileGraphError::GraphExportError { reason: e.to_string() })?;

        let text = resp.text().await
            .map_err(|e| tilegraph_core::TileGraphError::GraphExportError { reason: e.to_string() })?;

        let parsed: CypherResponse = serde_json::from_str(&text)
            .map_err(|e| tilegraph_core::TileGraphError::GraphExportError { reason: e.to_string() })?;

        if !parsed.errors.is_empty() {
            return Err(tilegraph_core::TileGraphError::GraphExportError {
                reason: format!("Neo4j errors: {:?}", parsed.errors),
            });
        }

        Ok(parsed)
    }

    pub async fn execute_batch(&self, statements: &[String]) -> Result<()> {
        for (i, stmt) in statements.iter().enumerate() {
            self.execute(stmt).await?;
            if i % 100 == 0 {
                tracing::debug!("Executed {}/{} statements", i, statements.len());
            }
        }
        Ok(())
    }
}
