use rusqlite::{Connection, OptionalExtension as _, params};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::cli_tools::CliToolId;

use super::{StorageError, now_ms, with_conn};

pub const PROMPT_DOCUMENT_MAX_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PromptScope {
    Global,
    Project,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptProject {
    pub id: String,
    pub name: String,
    pub path: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreatePromptProject {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdatePromptProject {
    pub name: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptDocument {
    pub tool: CliToolId,
    pub scope: PromptScope,
    pub project_id: Option<String>,
    pub content_md: String,
    pub exists: bool,
    pub created_at_ms: Option<i64>,
    pub updated_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SavePromptDocument {
    pub tool: CliToolId,
    pub scope: PromptScope,
    pub project_id: Option<String>,
    pub content_md: String,
    pub expected_updated_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeletePromptDocument {
    pub tool: CliToolId,
    pub scope: PromptScope,
    pub project_id: Option<String>,
    pub expected_updated_at_ms: Option<i64>,
}

#[derive(Debug, Clone)]
struct PromptDocumentRow {
    id: String,
    content_md: String,
    created_at_ms: i64,
    updated_at_ms: i64,
}

fn cli_tool_id_as_str(id: CliToolId) -> &'static str {
    match id {
        CliToolId::Gemini => "gemini",
        CliToolId::Claude => "claude",
        CliToolId::Codex => "codex",
    }
}

fn normalize_project_path(raw: &str) -> anyhow::Result<String> {
    let trimmed = raw.trim();
    anyhow::ensure!(!trimmed.is_empty(), "项目路径不能为空");

    let mut normalized = PathBuf::new();
    for component in PathBuf::from(trimmed).components() {
        normalized.push(component.as_os_str());
    }

    let out = normalized.to_string_lossy().trim().to_string();
    anyhow::ensure!(!out.is_empty(), "项目路径不能为空");
    Ok(out)
}

fn default_project_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|v| v.to_str())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(path)
        .to_string()
}

fn normalize_project_name(raw: &str, path: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        default_project_name(path)
    } else {
        trimmed.to_string()
    }
}

fn validate_document_size(content_md: &str) -> anyhow::Result<()> {
    let actual_bytes = content_md.len();
    if actual_bytes > PROMPT_DOCUMENT_MAX_BYTES {
        return Err(StorageError::PromptDocumentTooLarge {
            actual_bytes,
            max_bytes: PROMPT_DOCUMENT_MAX_BYTES,
        }
        .into());
    }
    Ok(())
}

fn scope_parts(
    scope: PromptScope,
    project_id: Option<String>,
) -> anyhow::Result<(String, Option<String>)> {
    match scope {
        PromptScope::Global => {
            anyhow::ensure!(project_id.is_none(), "全局提示词不允许携带 project_id");
            Ok(("global".to_string(), None))
        }
        PromptScope::Project => {
            let project_id = project_id
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .ok_or_else(|| anyhow::anyhow!("项目提示词缺少 project_id"))?;
            Ok((format!("project:{project_id}"), Some(project_id)))
        }
    }
}

fn ensure_project_exists(conn: &Connection, project_id: &str) -> anyhow::Result<()> {
    let exists = conn
        .query_row(
            r#"SELECT 1 FROM prompt_projects WHERE id = ?1"#,
            params![project_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?;

    if exists.is_none() {
        return Err(StorageError::PromptProjectNotFound {
            project_id: project_id.to_string(),
        }
        .into());
    }

    Ok(())
}

fn ensure_project_path_unique(
    conn: &Connection,
    path: &str,
    exclude_project_id: Option<&str>,
) -> anyhow::Result<()> {
    let exists = match exclude_project_id {
        Some(exclude_project_id) => conn
            .query_row(
                r#"SELECT 1 FROM prompt_projects WHERE path = ?1 AND id <> ?2"#,
                params![path, exclude_project_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?,
        None => conn
            .query_row(
                r#"SELECT 1 FROM prompt_projects WHERE path = ?1"#,
                params![path],
                |row| row.get::<_, i64>(0),
            )
            .optional()?,
    };

    if exists.is_some() {
        return Err(StorageError::PromptProjectPathExists {
            path: path.to_string(),
        }
        .into());
    }

    Ok(())
}

fn ensure_project_name_unique(
    conn: &Connection,
    name: &str,
    exclude_project_id: Option<&str>,
) -> anyhow::Result<()> {
    let exists = match exclude_project_id {
        Some(exclude_project_id) => conn
            .query_row(
                r#"SELECT 1 FROM prompt_projects WHERE name = ?1 COLLATE NOCASE AND id <> ?2"#,
                params![name, exclude_project_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?,
        None => conn
            .query_row(
                r#"SELECT 1 FROM prompt_projects WHERE name = ?1 COLLATE NOCASE"#,
                params![name],
                |row| row.get::<_, i64>(0),
            )
            .optional()?,
    };

    if exists.is_some() {
        return Err(StorageError::PromptProjectNameExists {
            name: name.to_string(),
        }
        .into());
    }

    Ok(())
}

fn fetch_prompt_document_row(
    conn: &Connection,
    tool: CliToolId,
    scope_key: &str,
) -> anyhow::Result<Option<PromptDocumentRow>> {
    conn.query_row(
        r#"
        SELECT id, content_md, created_at_ms, updated_at_ms
        FROM prompt_documents
        WHERE tool = ?1 AND scope_key = ?2
        "#,
        params![cli_tool_id_as_str(tool), scope_key],
        |row| {
            Ok(PromptDocumentRow {
                id: row.get(0)?,
                content_md: row.get(1)?,
                created_at_ms: row.get(2)?,
                updated_at_ms: row.get(3)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

fn ensure_document_version(
    existing_updated_at_ms: Option<i64>,
    expected_updated_at_ms: Option<i64>,
) -> anyhow::Result<()> {
    if existing_updated_at_ms == expected_updated_at_ms {
        return Ok(());
    }
    Err(StorageError::PromptDocumentVersionConflict {
        expected_updated_at_ms,
        current_updated_at_ms: existing_updated_at_ms,
    }
    .into())
}

fn touch_prompt_project(
    conn: &Connection,
    project_id: Option<&str>,
    touched_at_ms: i64,
) -> anyhow::Result<()> {
    if let Some(project_id) = project_id {
        conn.execute(
            r#"UPDATE prompt_projects SET updated_at_ms = ?2 WHERE id = ?1"#,
            params![project_id, touched_at_ms],
        )?;
    }
    Ok(())
}

pub async fn list_prompt_projects(db_path: PathBuf) -> anyhow::Result<Vec<PromptProject>> {
    with_conn(db_path, |conn| {
        let mut stmt = conn.prepare(
            r#"
            SELECT id, name, path, created_at_ms, updated_at_ms
            FROM prompt_projects
            ORDER BY updated_at_ms DESC, name COLLATE NOCASE ASC, path COLLATE NOCASE ASC
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(PromptProject {
                id: row.get(0)?,
                name: row.get(1)?,
                path: row.get(2)?,
                created_at_ms: row.get(3)?,
                updated_at_ms: row.get(4)?,
            })
        })?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    })
    .await
}

pub async fn create_prompt_project(
    db_path: PathBuf,
    input: CreatePromptProject,
) -> anyhow::Result<PromptProject> {
    with_conn(db_path, move |conn| {
        let path = normalize_project_path(&input.path)?;
        let name = normalize_project_name(&input.name, &path);

        ensure_project_path_unique(conn, &path, None)?;
        ensure_project_name_unique(conn, &name, None)?;

        let now = now_ms();
        let project = PromptProject {
            id: Uuid::new_v4().to_string(),
            name,
            path,
            created_at_ms: now,
            updated_at_ms: now,
        };

        conn.execute(
            r#"
            INSERT INTO prompt_projects (id, name, path, created_at_ms, updated_at_ms)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
            params![
                project.id,
                project.name,
                project.path,
                project.created_at_ms,
                project.updated_at_ms,
            ],
        )?;

        Ok(project)
    })
    .await
}

pub async fn update_prompt_project(
    db_path: PathBuf,
    project_id: String,
    input: UpdatePromptProject,
) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let existing = conn
            .query_row(
                r#"
                SELECT name, path
                FROM prompt_projects
                WHERE id = ?1
                "#,
                params![project_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;

        let Some((current_name, current_path)) = existing else {
            return Err(StorageError::PromptProjectNotFound { project_id }.into());
        };

        let path = normalize_project_path(input.path.as_deref().unwrap_or(&current_path))?;
        let name = normalize_project_name(input.name.as_deref().unwrap_or(&current_name), &path);

        ensure_project_path_unique(conn, &path, Some(&project_id))?;
        ensure_project_name_unique(conn, &name, Some(&project_id))?;

        conn.execute(
            r#"
            UPDATE prompt_projects
            SET name = ?2, path = ?3, updated_at_ms = ?4
            WHERE id = ?1
            "#,
            params![project_id, name, path, now_ms()],
        )?;

        Ok(())
    })
    .await
}

pub async fn delete_prompt_project(db_path: PathBuf, project_id: String) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let tx = conn.unchecked_transaction()?;

        tx.execute(
            r#"DELETE FROM prompt_documents WHERE project_id = ?1"#,
            params![project_id],
        )?;

        let changed: i64 = tx
            .execute(
                r#"DELETE FROM prompt_projects WHERE id = ?1"#,
                params![project_id],
            )?
            .try_into()?;

        if changed == 0 {
            return Err(StorageError::PromptProjectNotFound { project_id }.into());
        }

        tx.commit()?;
        Ok(())
    })
    .await
}

pub async fn get_prompt_document(
    db_path: PathBuf,
    tool: CliToolId,
    scope: PromptScope,
    project_id: Option<String>,
) -> anyhow::Result<PromptDocument> {
    with_conn(db_path, move |conn| {
        let (scope_key, project_id) = scope_parts(scope, project_id)?;
        if let Some(project_id) = project_id.as_deref() {
            ensure_project_exists(conn, project_id)?;
        }

        let row = fetch_prompt_document_row(conn, tool, &scope_key)?;

        Ok(match row {
            Some(row) => PromptDocument {
                tool,
                scope,
                project_id,
                content_md: row.content_md,
                exists: true,
                created_at_ms: Some(row.created_at_ms),
                updated_at_ms: Some(row.updated_at_ms),
            },
            None => PromptDocument {
                tool,
                scope,
                project_id,
                content_md: String::new(),
                exists: false,
                created_at_ms: None,
                updated_at_ms: None,
            },
        })
    })
    .await
}

pub async fn save_prompt_document(
    db_path: PathBuf,
    input: SavePromptDocument,
) -> anyhow::Result<PromptDocument> {
    with_conn(db_path, move |conn| {
        validate_document_size(&input.content_md)?;

        let (scope_key, project_id) = scope_parts(input.scope, input.project_id)?;
        if let Some(project_id) = project_id.as_deref() {
            ensure_project_exists(conn, project_id)?;
        }

        let now = now_ms();
        let tx = conn.unchecked_transaction()?;
        let existing = fetch_prompt_document_row(&tx, input.tool, &scope_key)?;

        let (created_at_ms, updated_at_ms) = match existing {
            Some(existing) => {
                ensure_document_version(
                    Some(existing.updated_at_ms),
                    input.expected_updated_at_ms,
                )?;
                tx.execute(
                    r#"
                    UPDATE prompt_documents
                    SET content_md = ?2, project_id = ?3, updated_at_ms = ?4
                    WHERE id = ?1
                    "#,
                    params![existing.id, input.content_md, project_id, now],
                )?;
                (existing.created_at_ms, now)
            }
            None => {
                ensure_document_version(None, input.expected_updated_at_ms)?;
                tx.execute(
                    r#"
                    INSERT INTO prompt_documents (
                      id, tool, scope, scope_key, project_id, content_md, created_at_ms, updated_at_ms
                    )
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                    "#,
                    params![
                        Uuid::new_v4().to_string(),
                        cli_tool_id_as_str(input.tool),
                        match input.scope {
                            PromptScope::Global => "global",
                            PromptScope::Project => "project",
                        },
                        scope_key,
                        project_id,
                        input.content_md,
                        now,
                        now,
                    ],
                )?;
                (now, now)
            }
        };

        touch_prompt_project(&tx, project_id.as_deref(), now)?;
        tx.commit()?;

        Ok(PromptDocument {
            tool: input.tool,
            scope: input.scope,
            project_id,
            content_md: input.content_md,
            exists: true,
            created_at_ms: Some(created_at_ms),
            updated_at_ms: Some(updated_at_ms),
        })
    })
    .await
}

pub async fn delete_prompt_document(
    db_path: PathBuf,
    input: DeletePromptDocument,
) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let (scope_key, project_id) = scope_parts(input.scope, input.project_id)?;
        if let Some(project_id) = project_id.as_deref() {
            ensure_project_exists(conn, project_id)?;
        }

        let now = now_ms();
        let tx = conn.unchecked_transaction()?;
        let existing = fetch_prompt_document_row(&tx, input.tool, &scope_key)?
            .ok_or(StorageError::PromptDocumentNotFound)?;

        ensure_document_version(Some(existing.updated_at_ms), input.expected_updated_at_ms)?;

        tx.execute(
            r#"DELETE FROM prompt_documents WHERE id = ?1"#,
            params![existing.id],
        )?;
        touch_prompt_project(&tx, project_id.as_deref(), now)?;
        tx.commit()?;
        Ok(())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage;

    fn temp_db_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "cliswitch-prompt-tests-{name}-{}.sqlite",
            uuid::Uuid::new_v4()
        ))
    }

    fn cleanup_db_files(db_path: &Path) {
        let _ = std::fs::remove_file(db_path);
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-shm"));
        let wal = format!("{}-wal", db_path.display());
        let shm = format!("{}-shm", db_path.display());
        let _ = std::fs::remove_file(wal);
        let _ = std::fs::remove_file(shm);
    }

    #[tokio::test]
    async fn prompt_project_name_must_be_unique_case_insensitive() {
        let db_path = temp_db_path("name-unique");
        storage::init_db(&db_path).unwrap();

        let p1 = create_prompt_project(
            db_path.clone(),
            CreatePromptProject {
                name: "CliSwitch".to_string(),
                path: "/tmp/project-a".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(p1.name, "CliSwitch");

        let err = create_prompt_project(
            db_path.clone(),
            CreatePromptProject {
                name: "cliswitch".to_string(),
                path: "/tmp/project-b".to_string(),
            },
        )
        .await
        .unwrap_err();

        assert!(matches!(
            err.downcast_ref::<StorageError>(),
            Some(StorageError::PromptProjectNameExists { .. })
        ));

        cleanup_db_files(&db_path);
    }

    #[tokio::test]
    async fn save_prompt_document_rejects_oversized_content() {
        let db_path = temp_db_path("too-large");
        storage::init_db(&db_path).unwrap();

        let err = save_prompt_document(
            db_path.clone(),
            SavePromptDocument {
                tool: CliToolId::Codex,
                scope: PromptScope::Global,
                project_id: None,
                content_md: "a".repeat(PROMPT_DOCUMENT_MAX_BYTES + 1),
                expected_updated_at_ms: None,
            },
        )
        .await
        .unwrap_err();

        assert!(matches!(
            err.downcast_ref::<StorageError>(),
            Some(StorageError::PromptDocumentTooLarge { .. })
        ));

        cleanup_db_files(&db_path);
    }

    #[tokio::test]
    async fn prompt_document_uses_optimistic_lock_and_can_delete() {
        let db_path = temp_db_path("optimistic-lock");
        storage::init_db(&db_path).unwrap();

        let first = save_prompt_document(
            db_path.clone(),
            SavePromptDocument {
                tool: CliToolId::Codex,
                scope: PromptScope::Global,
                project_id: None,
                content_md: "hello".to_string(),
                expected_updated_at_ms: None,
            },
        )
        .await
        .unwrap();

        let second = save_prompt_document(
            db_path.clone(),
            SavePromptDocument {
                tool: CliToolId::Codex,
                scope: PromptScope::Global,
                project_id: None,
                content_md: "world".to_string(),
                expected_updated_at_ms: first.updated_at_ms,
            },
        )
        .await
        .unwrap();

        let err = save_prompt_document(
            db_path.clone(),
            SavePromptDocument {
                tool: CliToolId::Codex,
                scope: PromptScope::Global,
                project_id: None,
                content_md: "stale".to_string(),
                expected_updated_at_ms: first.updated_at_ms,
            },
        )
        .await
        .unwrap_err();

        assert!(matches!(
            err.downcast_ref::<StorageError>(),
            Some(StorageError::PromptDocumentVersionConflict { .. })
        ));

        delete_prompt_document(
            db_path.clone(),
            DeletePromptDocument {
                tool: CliToolId::Codex,
                scope: PromptScope::Global,
                project_id: None,
                expected_updated_at_ms: second.updated_at_ms,
            },
        )
        .await
        .unwrap();

        let doc = get_prompt_document(db_path.clone(), CliToolId::Codex, PromptScope::Global, None)
            .await
            .unwrap();
        assert!(!doc.exists);
        assert!(doc.content_md.is_empty());

        cleanup_db_files(&db_path);
    }
}
