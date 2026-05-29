/// HTTP client for Neo4j Bolt HTTP API (V1 uses Cypher HTTP endpoint).
/// Production would use bolt:// driver. V1 uses HTTP for portability.
use serde::{Deserialize, Serialize};
use tilegraph_core::Result;

#[derive(Debug, Clone)]
pub struct Neo4jConfig {
    pub url: String, // e.g. http://localhost:7474
    pub username: String,
    pub password: String,
    pub database: String, // e.g. "neo4j"
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
    pub config: Neo4jConfig,
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
        let url = format!("{}/db/{}/tx/commit", self.config.url, self.config.database);
        let body = CypherRequest {
            statements: vec![Statement {
                statement: cypher.to_string(),
            }],
        };

        let resp = self
            .http
            .post(&url)
            .basic_auth(&self.config.username, Some(&self.config.password))
            .json(&body)
            .send()
            .await
            .map_err(|e| tilegraph_core::TileGraphError::GraphExportError {
                reason: e.to_string(),
            })?;

        let text =
            resp.text()
                .await
                .map_err(|e| tilegraph_core::TileGraphError::GraphExportError {
                    reason: e.to_string(),
                })?;

        let parsed: CypherResponse = serde_json::from_str(&text).map_err(|e| {
            tilegraph_core::TileGraphError::GraphExportError {
                reason: e.to_string(),
            }
        })?;

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

    /// Execute statements in parallel batches.
    /// `batch_size`: statements per transaction; `parallelism`: max concurrent transactions.
    pub async fn execute_parallel_batch(
        &self,
        statements: &[String],
        batch_size: usize,
        parallelism: usize,
    ) -> Result<usize> {
        if statements.is_empty() {
            return Ok(0);
        }

        let chunks: Vec<Vec<String>> = statements
            .chunks(batch_size.max(1))
            .map(|c| c.to_vec())
            .collect();

        let total_chunks = chunks.len();
        let mut executed = 0usize;
        let mut chunk_iter = chunks.into_iter();

        loop {
            let mut join_set = tokio::task::JoinSet::new();
            let mut batch_taken = 0;

            while batch_taken < parallelism {
                match chunk_iter.next() {
                    Some(chunk) => {
                        let http = self.http.clone();
                        let config = self.config.clone();
                        join_set
                            .spawn(async move { execute_chunk_http(&http, &config, &chunk).await });
                        batch_taken += 1;
                    }
                    None => break,
                }
            }

            if join_set.is_empty() {
                break;
            }

            while let Some(result) = join_set.join_next().await {
                result.map_err(|e| tilegraph_core::TileGraphError::GraphExportError {
                    reason: format!("Task join error: {}", e),
                })??;
                executed += 1;
                if executed % 10 == 0 {
                    tracing::info!("Neo4j import: {}/{} batches", executed, total_chunks);
                }
            }
        }

        tracing::info!(
            "Neo4j import complete: {}/{} batches, {} statements",
            executed,
            total_chunks,
            statements.len()
        );
        Ok(executed)
    }
}

async fn execute_chunk_http(
    http: &reqwest::Client,
    config: &Neo4jConfig,
    statements: &[String],
) -> Result<()> {
    let body = serde_json::json!({
        "statements": statements.iter()
            .map(|s| serde_json::json!({ "statement": s }))
            .collect::<Vec<_>>()
    });

    let url = format!("{}/db/{}/tx/commit", config.url, config.database);
    let resp = http
        .post(&url)
        .basic_auth(&config.username, Some(&config.password))
        .json(&body)
        .send()
        .await
        .map_err(|e| tilegraph_core::TileGraphError::GraphExportError {
            reason: e.to_string(),
        })?;

    let parsed: serde_json::Value =
        resp.json()
            .await
            .map_err(|e| tilegraph_core::TileGraphError::GraphExportError {
                reason: e.to_string(),
            })?;

    if let Some(errors) = parsed["errors"].as_array() {
        if !errors.is_empty() {
            return Err(tilegraph_core::TileGraphError::GraphExportError {
                reason: format!("Neo4j errors: {:?}", errors),
            });
        }
    }
    Ok(())
}
