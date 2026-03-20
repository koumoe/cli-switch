use anyhow::Context as _;
use directories::UserDirs;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::chat_bridge::i18n::{args, t, t_args};
use crate::cli_tools::CliToolId;
use crate::i18n::AppLocale;
use crate::storage;

const PROJECT_SELECTION_TTL_MS: i64 = 10 * 60 * 1000;
pub const PROJECT_SELECTION_DISPLAY_INDEX_START: usize = 1001;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProjectSelectionCacheKey {
    pub platform: String,
    pub chat_id: String,
}

#[derive(Debug, Clone)]
pub struct AggregatedProject {
    pub path: String,
    pub display_name: String,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone)]
struct ProjectSelectionSnapshot {
    items: Vec<AggregatedProject>,
    expires_at_ms: i64,
}

#[derive(Default)]
pub struct ProjectStore {
    selection_cache:
        tokio::sync::RwLock<HashMap<ProjectSelectionCacheKey, ProjectSelectionSnapshot>>,
}

impl ProjectStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn list_all_projects(
        &self,
        db_path: PathBuf,
    ) -> anyhow::Result<Vec<AggregatedProject>> {
        let (claude, codex, gemini, known) = tokio::join!(
            storage::list_prompt_projects(db_path.clone(), CliToolId::Claude),
            storage::list_prompt_projects(db_path.clone(), CliToolId::Codex),
            storage::list_prompt_projects(db_path.clone(), CliToolId::Gemini),
            storage::list_bridge_known_projects(db_path),
        );

        let mut merged = HashMap::<String, AggregatedProjectEntry>::new();

        for result in [claude, codex, gemini] {
            for item in result? {
                let entry =
                    merged
                        .entry(item.path.clone())
                        .or_insert_with(|| AggregatedProjectEntry {
                            path: item.path.clone(),
                            display_name: item.name.clone(),
                            updated_at_ms: item.updated_at_ms,
                        });
                entry.display_name = choose_display_name(&entry.display_name, &item.name);
                entry.updated_at_ms = entry.updated_at_ms.max(item.updated_at_ms);
            }
        }

        for item in known? {
            let entry = merged
                .entry(item.path.clone())
                .or_insert_with(|| AggregatedProjectEntry {
                    path: item.path.clone(),
                    display_name: item.display_name.clone(),
                    updated_at_ms: item.updated_at_ms,
                });
            entry.display_name = choose_display_name(&entry.display_name, &item.display_name);
            entry.updated_at_ms = entry.updated_at_ms.max(item.updated_at_ms);
        }

        let mut items = merged
            .into_values()
            .map(|entry| AggregatedProject {
                path: entry.path,
                display_name: entry.display_name,
                updated_at_ms: entry.updated_at_ms,
            })
            .collect::<Vec<_>>();

        items.sort_by(|left, right| {
            right
                .updated_at_ms
                .cmp(&left.updated_at_ms)
                .then_with(|| {
                    left.display_name
                        .to_lowercase()
                        .cmp(&right.display_name.to_lowercase())
                })
                .then_with(|| left.path.to_lowercase().cmp(&right.path.to_lowercase()))
        });

        Ok(items)
    }

    pub async fn remember_snapshot(
        &self,
        key: ProjectSelectionCacheKey,
        items: Vec<AggregatedProject>,
    ) {
        let expires_at_ms = storage::now_ms().saturating_add(PROJECT_SELECTION_TTL_MS);
        self.selection_cache.write().await.insert(
            key,
            ProjectSelectionSnapshot {
                items,
                expires_at_ms,
            },
        );
    }

    pub async fn resolve_project_ref(
        &self,
        db_path: PathBuf,
        key: &ProjectSelectionCacheKey,
        project_ref: &str,
        allow_new_projects: bool,
        locale: AppLocale,
    ) -> anyhow::Result<AggregatedProject> {
        let reference = project_ref.trim();
        anyhow::ensure!(!reference.is_empty(), "project reference is required");

        if let Ok(index) = reference.parse::<usize>() {
            if index < PROJECT_SELECTION_DISPLAY_INDEX_START {
                anyhow::bail!(
                    "{}",
                    t_args(
                        locale,
                        "project.index_start",
                        &args([("start", PROJECT_SELECTION_DISPLAY_INDEX_START.to_string())]),
                    )
                );
            }
            let cache = self.selection_cache.read().await;
            let Some(snapshot) = cache.get(key) else {
                anyhow::bail!("{}", t(locale, "project.index_missing"));
            };
            if snapshot.expires_at_ms < storage::now_ms() {
                anyhow::bail!("{}", t(locale, "project.index_expired"));
            }
            let pos = index - PROJECT_SELECTION_DISPLAY_INDEX_START;
            return snapshot
                .items
                .get(pos)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("{}", t(locale, "project.index_not_found")));
        }

        if let Some(path) = normalize_explicit_project_path(reference) {
            let canonical = if allow_new_projects {
                ensure_project_dir(&path, locale)?
            } else {
                canonical_existing_dir(&path).ok_or_else(|| {
                    anyhow::anyhow!(
                        "{}",
                        t_args(
                            locale,
                            "project.path_not_found",
                            &args([("path", path.display().to_string())]),
                        )
                    )
                })?
            };
            let canonical_str = canonical.to_string_lossy().to_string();
            let projects = self.list_all_projects(db_path.clone()).await?;
            if let Some(existing) = projects
                .into_iter()
                .find(|item| item.path.eq_ignore_ascii_case(&canonical_str))
            {
                return Ok(existing);
            }

            if !allow_new_projects {
                anyhow::bail!("{}", t(locale, "project.path_unknown"));
            }

            let known =
                storage::upsert_bridge_known_project(db_path, canonical_str.clone(), None).await?;
            return Ok(AggregatedProject {
                path: known.path,
                display_name: known.display_name,
                updated_at_ms: known.updated_at_ms,
            });
        }

        let lowered = reference.to_lowercase();
        let matches = self
            .list_all_projects(db_path)
            .await?
            .into_iter()
            .filter(|item| item.display_name.to_lowercase() == lowered)
            .collect::<Vec<_>>();
        match matches.len() {
            0 => anyhow::bail!(
                "{}",
                t_args(
                    locale,
                    "project.not_found",
                    &args([("reference", reference.to_string())]),
                )
            ),
            1 => Ok(matches.into_iter().next().unwrap_or_else(|| unreachable!())),
            _ => anyhow::bail!("{}", t(locale, "project.ambiguous")),
        }
    }
}

pub fn display_project_index(offset: usize) -> usize {
    PROJECT_SELECTION_DISPLAY_INDEX_START + offset
}

#[derive(Debug)]
struct AggregatedProjectEntry {
    path: String,
    display_name: String,
    updated_at_ms: i64,
}

fn choose_display_name(existing: &str, candidate: &str) -> String {
    if !existing.trim().is_empty() {
        return existing.to_string();
    }
    if !candidate.trim().is_empty() {
        return candidate.to_string();
    }
    existing.to_string()
}

fn normalize_explicit_project_path(raw: &str) -> Option<PathBuf> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.starts_with("~/") || trimmed == "~" {
        let home_dir = UserDirs::new()?.home_dir().to_path_buf();
        if trimmed == "~" {
            return Some(home_dir);
        }
        return Some(home_dir.join(trimmed.trim_start_matches("~/")));
    }

    let path = PathBuf::from(trimmed);
    path.is_absolute().then_some(path)
}

fn canonical_existing_dir(path: &Path) -> Option<PathBuf> {
    let canonical = std::fs::canonicalize(path).ok()?;
    canonical.is_dir().then_some(canonical)
}

fn ensure_project_dir(path: &Path, locale: AppLocale) -> anyhow::Result<PathBuf> {
    if path.exists() {
        return canonical_existing_dir(path).ok_or_else(|| {
            anyhow::anyhow!(
                "{}",
                t_args(
                    locale,
                    "project.path_not_dir",
                    &args([("path", path.display().to_string())]),
                )
            )
        });
    }

    std::fs::create_dir_all(path).with_context(|| {
        t_args(
            locale,
            "project.path_create_failed",
            &args([("path", path.display().to_string())]),
        )
    })?;
    canonical_existing_dir(path).ok_or_else(|| {
        anyhow::anyhow!(
            "{}",
            t_args(
                locale,
                "project.path_canonicalize_failed",
                &args([("path", path.display().to_string())]),
            )
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn remove_sqlite_artifacts(path: &Path) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(PathBuf::from(format!("{}-wal", path.display())));
        let _ = std::fs::remove_file(PathBuf::from(format!("{}-shm", path.display())));
    }

    fn temp_db_path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "cliswitch-test-chat-bridge-projects-{}.sqlite",
            uuid::Uuid::new_v4()
        ))
    }

    fn temp_project_root() -> PathBuf {
        std::env::temp_dir().join(format!(
            "cliswitch-test-chat-bridge-project-dir-{}",
            uuid::Uuid::new_v4()
        ))
    }

    #[tokio::test]
    async fn resolve_project_ref_creates_missing_directory_when_allowed() {
        let db_path = temp_db_path();
        let project_root = temp_project_root();
        remove_sqlite_artifacts(&db_path);
        let _ = std::fs::remove_dir_all(&project_root);
        crate::storage::init_db(&db_path).expect("init db");

        let project_path = project_root.join("aaa");
        let store = ProjectStore::new();
        let project = store
            .resolve_project_ref(
                db_path.clone(),
                &ProjectSelectionCacheKey {
                    platform: "telegram".to_string(),
                    chat_id: "chat-1".to_string(),
                },
                &project_path.to_string_lossy(),
                true,
                AppLocale::ZhCN,
            )
            .await
            .expect("resolve project ref");

        assert!(project_path.is_dir());
        assert_eq!(
            project.path,
            std::fs::canonicalize(&project_path)
                .expect("canonical path")
                .to_string_lossy()
                .to_string()
        );
        assert_eq!(project.display_name, "aaa");

        let known = crate::storage::list_bridge_known_projects(db_path.clone())
            .await
            .expect("list known projects");
        assert_eq!(known.len(), 1);
        assert_eq!(known[0].path, project.path);

        let _ = std::fs::remove_dir_all(&project_root);
        remove_sqlite_artifacts(&db_path);
    }

    #[tokio::test]
    async fn resolve_project_ref_uses_display_indexes_starting_from_1001() {
        let db_path = temp_db_path();
        remove_sqlite_artifacts(&db_path);
        crate::storage::init_db(&db_path).expect("init db");
        let store = ProjectStore::new();
        let key = ProjectSelectionCacheKey {
            platform: "telegram".to_string(),
            chat_id: "chat-1".to_string(),
        };
        store
            .remember_snapshot(
                key.clone(),
                vec![AggregatedProject {
                    path: "/tmp/demo".to_string(),
                    display_name: "demo".to_string(),
                    updated_at_ms: 1,
                }],
            )
            .await;

        let project = store
            .resolve_project_ref(db_path.clone(), &key, "1001", false, AppLocale::ZhCN)
            .await
            .expect("resolve display index");
        assert_eq!(project.display_name, "demo");

        let err = store
            .resolve_project_ref(db_path.clone(), &key, "1", false, AppLocale::ZhCN)
            .await
            .expect_err("legacy index should fail");
        assert!(err.to_string().contains("项目编号从 1001 开始"));

        remove_sqlite_artifacts(&db_path);
    }
}
