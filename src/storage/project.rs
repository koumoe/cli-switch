use anyhow::Context as _;
use directories::UserDirs;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::cli_tools::CliToolId;

use super::StorageError;

pub const PROJECT_DOCUMENT_MAX_BYTES: usize = 256 * 1024;

const CLAUDE_PROMPT_FILENAME: &str = "CLAUDE.md";
const CODEX_PROMPT_FILENAME: &str = "AGENTS.md";
const GEMINI_PROMPT_FILENAME: &str = "GEMINI.md";
const SESSION_SCAN_MAX_LINES: usize = 16;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProjectScope {
    Global,
    Project,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRecord {
    pub id: String,
    pub name: String,
    pub path: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDocument {
    pub tool: CliToolId,
    pub scope: ProjectScope,
    pub project_id: Option<String>,
    pub content_md: String,
    pub exists: bool,
    pub created_at_ms: Option<i64>,
    pub updated_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SaveProjectDocument {
    pub tool: CliToolId,
    pub scope: ProjectScope,
    pub project_id: Option<String>,
    pub content_md: String,
    pub expected_updated_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeleteProjectDocument {
    pub tool: CliToolId,
    pub scope: ProjectScope,
    pub project_id: Option<String>,
    pub expected_updated_at_ms: Option<i64>,
}

#[derive(Debug, Clone)]
struct DiscoveredProject {
    root: PathBuf,
    updated_at_ms: i64,
}

#[derive(Debug, Clone)]
struct ProjectFileState {
    content_md: String,
    created_at_ms: i64,
    updated_at_ms: i64,
}

fn user_home_dir() -> anyhow::Result<PathBuf> {
    let user_dirs = UserDirs::new().context("读取用户目录失败")?;
    Ok(user_dirs.home_dir().to_path_buf())
}

fn claude_home_dir() -> anyhow::Result<PathBuf> {
    if let Some(path) = std::env::var_os("CLAUDE_CONFIG_DIR") {
        return Ok(PathBuf::from(path));
    }
    Ok(user_home_dir()?.join(".claude"))
}

fn codex_home_dir() -> anyhow::Result<PathBuf> {
    if let Some(path) = std::env::var_os("CODEX_HOME") {
        return Ok(PathBuf::from(path));
    }
    Ok(user_home_dir()?.join(".codex"))
}

fn gemini_home_dir() -> anyhow::Result<PathBuf> {
    Ok(user_home_dir()?.join(".gemini"))
}

fn tool_project_document_filename(tool: CliToolId) -> &'static str {
    match tool {
        CliToolId::Claude => CLAUDE_PROMPT_FILENAME,
        CliToolId::Codex => CODEX_PROMPT_FILENAME,
        CliToolId::Gemini => GEMINI_PROMPT_FILENAME,
    }
}

fn global_project_document_path(tool: CliToolId) -> anyhow::Result<PathBuf> {
    match tool {
        CliToolId::Claude => Ok(claude_home_dir()?.join(CLAUDE_PROMPT_FILENAME)),
        CliToolId::Codex => Ok(codex_home_dir()?.join(CODEX_PROMPT_FILENAME)),
        CliToolId::Gemini => Ok(gemini_home_dir()?.join(GEMINI_PROMPT_FILENAME)),
    }
}

fn system_time_to_ms(ts: SystemTime) -> Option<i64> {
    let duration = ts.duration_since(UNIX_EPOCH).ok()?;
    i64::try_from(duration.as_millis()).ok()
}

fn path_modified_ms(path: &Path) -> Option<i64> {
    let metadata = fs::metadata(path).ok()?;
    let modified = metadata.modified().ok()?;
    system_time_to_ms(modified)
}

fn normalize_existing_dir(path: &Path) -> Option<PathBuf> {
    let canonical = fs::canonicalize(path).ok()?;
    canonical.is_dir().then_some(canonical)
}

fn find_git_root(start: &Path) -> Option<PathBuf> {
    let mut current = Some(start);
    while let Some(dir) = current {
        if dir.join(".git").exists() {
            return Some(dir.to_path_buf());
        }
        current = dir.parent();
    }
    None
}

fn is_filesystem_root(path: &Path) -> bool {
    path.parent().is_none()
}

fn canonical_project_root(path: &Path) -> Option<PathBuf> {
    let dir = normalize_existing_dir(path)?;
    let root = find_git_root(&dir).unwrap_or(dir);
    let canonical_root = fs::canonicalize(root).ok()?;
    if !canonical_root.is_dir() || is_filesystem_root(&canonical_root) {
        return None;
    }
    Some(canonical_root)
}

fn default_project_name(path: &Path) -> String {
    if let Some(name) = path
        .file_name()
        .and_then(|value| value.to_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return name.to_string();
    }

    path.to_string_lossy().to_string()
}

fn validate_document_size(content_md: &str) -> anyhow::Result<()> {
    let actual_bytes = content_md.len();
    if actual_bytes > PROJECT_DOCUMENT_MAX_BYTES {
        return Err(StorageError::ProjectDocumentTooLarge {
            actual_bytes,
            max_bytes: PROJECT_DOCUMENT_MAX_BYTES,
        }
        .into());
    }
    Ok(())
}

fn ensure_document_version(
    existing_updated_at_ms: Option<i64>,
    expected_updated_at_ms: Option<i64>,
) -> anyhow::Result<()> {
    if existing_updated_at_ms == expected_updated_at_ms {
        return Ok(());
    }
    Err(StorageError::ProjectDocumentVersionConflict {
        expected_updated_at_ms,
        current_updated_at_ms: existing_updated_at_ms,
    }
    .into())
}

fn visit_files_recursively(root: &Path, visitor: &mut impl FnMut(&Path)) -> anyhow::Result<()> {
    if !root.exists() {
        return Ok(());
    }

    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(err) => {
            tracing::warn!(path = %root.display(), err = %err, "failed to read project discovery dir");
            return Ok(());
        }
    };

    for entry in entries {
        let Ok(entry) = entry else { continue };
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let path = entry.path();

        if file_type.is_symlink() {
            continue;
        }

        if file_type.is_dir() {
            visit_files_recursively(&path, visitor)?;
            continue;
        }

        if file_type.is_file() {
            visitor(&path);
        }
    }

    Ok(())
}

fn extract_cwd_from_json_value(value: &Value) -> Option<String> {
    value
        .get("cwd")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            value
                .get("payload")
                .and_then(|payload| payload.get("cwd"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
        })
}

fn extract_cwd_from_jsonl(path: &Path) -> Option<String> {
    let file = fs::File::open(path).ok()?;
    let reader = BufReader::new(file);

    for line in reader.lines().take(SESSION_SCAN_MAX_LINES) {
        let Ok(line) = line else { continue };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(trimmed) else {
            continue;
        };
        if let Some(cwd) = extract_cwd_from_json_value(&value) {
            return Some(cwd);
        }
    }

    None
}

fn merge_discovered_project(
    projects: &mut HashMap<String, DiscoveredProject>,
    tool: CliToolId,
    raw_root: &Path,
    source_updated_at_ms: i64,
) {
    let Some(root) = canonical_project_root(raw_root) else {
        return;
    };

    let document_path = root.join(tool_project_document_filename(tool));
    let updated_at_ms = source_updated_at_ms.max(path_modified_ms(&document_path).unwrap_or(0));
    let path_key = root.to_string_lossy().to_string();

    match projects.get_mut(&path_key) {
        Some(existing) => {
            existing.updated_at_ms = existing.updated_at_ms.max(updated_at_ms);
        }
        None => {
            projects.insert(
                path_key,
                DiscoveredProject {
                    root,
                    updated_at_ms,
                },
            );
        }
    }
}

fn discover_claude_projects_in_dir(projects_root: &Path) -> anyhow::Result<Vec<DiscoveredProject>> {
    let mut projects = HashMap::new();

    visit_files_recursively(projects_root, &mut |path| {
        if path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
            return;
        }
        let Some(cwd) = extract_cwd_from_jsonl(path) else {
            return;
        };
        let updated_at_ms = path_modified_ms(path).unwrap_or(0);
        merge_discovered_project(
            &mut projects,
            CliToolId::Claude,
            Path::new(&cwd),
            updated_at_ms,
        );
    })?;

    Ok(projects.into_values().collect())
}

fn discover_codex_projects_in_dir(sessions_root: &Path) -> anyhow::Result<Vec<DiscoveredProject>> {
    let mut projects = HashMap::new();

    visit_files_recursively(sessions_root, &mut |path| {
        if path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
            return;
        }
        let Some(cwd) = extract_cwd_from_jsonl(path) else {
            return;
        };
        let updated_at_ms = path_modified_ms(path).unwrap_or(0);
        merge_discovered_project(
            &mut projects,
            CliToolId::Codex,
            Path::new(&cwd),
            updated_at_ms,
        );
    })?;

    Ok(projects.into_values().collect())
}

fn discover_gemini_projects_in_dir(gemini_root: &Path) -> anyhow::Result<Vec<DiscoveredProject>> {
    let mut projects = HashMap::new();

    let projects_json = gemini_root.join("projects.json");
    if let Ok(raw) = fs::read_to_string(&projects_json)
        && let Ok(value) = serde_json::from_str::<Value>(&raw)
        && let Some(items) = value.get("projects").and_then(Value::as_object)
    {
        let updated_at_ms = path_modified_ms(&projects_json).unwrap_or(0);
        for path in items.keys() {
            merge_discovered_project(
                &mut projects,
                CliToolId::Gemini,
                Path::new(path),
                updated_at_ms,
            );
        }
    }

    for marker_root in [gemini_root.join("history"), gemini_root.join("tmp")] {
        visit_files_recursively(&marker_root, &mut |path| {
            if path.file_name().and_then(|value| value.to_str()) != Some(".project_root") {
                return;
            }
            let Ok(raw) = fs::read_to_string(path) else {
                return;
            };
            let project_root = raw.trim();
            if project_root.is_empty() {
                return;
            }
            let updated_at_ms = path_modified_ms(path).unwrap_or(0);
            merge_discovered_project(
                &mut projects,
                CliToolId::Gemini,
                Path::new(project_root),
                updated_at_ms,
            );
        })?;
    }

    Ok(projects.into_values().collect())
}

fn discover_projects(tool: CliToolId) -> anyhow::Result<Vec<DiscoveredProject>> {
    match tool {
        CliToolId::Claude => discover_claude_projects_in_dir(&claude_home_dir()?.join("projects")),
        CliToolId::Codex => discover_codex_projects_in_dir(&codex_home_dir()?.join("sessions")),
        CliToolId::Gemini => discover_gemini_projects_in_dir(&gemini_home_dir()?),
    }
}

fn project_root_matches(path: &Path, target_root: &Path) -> bool {
    canonical_project_root(path)
        .map(|root| root == target_root)
        .unwrap_or(false)
}

fn collect_session_files_for_project(
    root: &Path,
    target_root: &Path,
) -> anyhow::Result<Vec<PathBuf>> {
    let mut matches = Vec::new();

    visit_files_recursively(root, &mut |path| {
        if path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
            return;
        }
        let Some(cwd) = extract_cwd_from_jsonl(path) else {
            return;
        };
        if project_root_matches(Path::new(&cwd), target_root) {
            matches.push(path.to_path_buf());
        }
    })?;

    Ok(matches)
}

fn remove_files(paths: &[PathBuf], kind: &str) -> anyhow::Result<()> {
    for path in paths {
        fs::remove_file(path).with_context(|| format!("删除 {kind} 失败：{}", path.display()))?;
    }
    Ok(())
}

fn remove_gemini_project_entries(gemini_root: &Path, target_root: &Path) -> anyhow::Result<()> {
    let projects_json = gemini_root.join("projects.json");
    match fs::read_to_string(&projects_json) {
        Ok(raw) => match serde_json::from_str::<Value>(&raw) {
            Ok(mut value) => {
                let mut changed = false;
                if let Some(items) = value.get_mut("projects").and_then(Value::as_object_mut) {
                    let keys_to_remove = items
                        .keys()
                        .filter(|path| project_root_matches(Path::new(path), target_root))
                        .cloned()
                        .collect::<Vec<_>>();

                    for key in keys_to_remove {
                        changed |= items.remove(&key).is_some();
                    }
                }

                if changed {
                    let serialized = serde_json::to_string_pretty(&value)
                        .context("序列化 Gemini projects.json 失败")?;
                    fs::write(&projects_json, serialized).with_context(|| {
                        format!(
                            "写入 Gemini projects.json 失败：{}",
                            projects_json.display()
                        )
                    })?;
                }
            }
            Err(err) => {
                tracing::warn!(
                    path = %projects_json.display(),
                    err = %err,
                    "failed to parse gemini projects.json during project cleanup"
                );
            }
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => {
            tracing::warn!(
                path = %projects_json.display(),
                err = %err,
                "failed to read gemini projects.json during project cleanup"
            );
        }
    }

    let mut marker_files = Vec::new();
    for marker_root in [gemini_root.join("history"), gemini_root.join("tmp")] {
        visit_files_recursively(&marker_root, &mut |path| {
            if path.file_name().and_then(|value| value.to_str()) != Some(".project_root") {
                return;
            }
            let Ok(raw) = fs::read_to_string(path) else {
                return;
            };
            if project_root_matches(Path::new(raw.trim()), target_root) {
                marker_files.push(path.to_path_buf());
            }
        })?;
    }

    remove_files(&marker_files, "Gemini 项目标记文件")
}

fn sort_projects(items: &mut [ProjectRecord]) {
    items.sort_by(|left, right| {
        right
            .updated_at_ms
            .cmp(&left.updated_at_ms)
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
            .then_with(|| left.path.to_lowercase().cmp(&right.path.to_lowercase()))
    });
}

fn project_document_from_file(
    tool: CliToolId,
    scope: ProjectScope,
    project_id: Option<String>,
    state: Option<ProjectFileState>,
) -> ProjectDocument {
    match state {
        Some(state) => ProjectDocument {
            tool,
            scope,
            project_id,
            content_md: state.content_md,
            exists: true,
            created_at_ms: Some(state.created_at_ms),
            updated_at_ms: Some(state.updated_at_ms),
        },
        None => ProjectDocument {
            tool,
            scope,
            project_id,
            content_md: String::new(),
            exists: false,
            created_at_ms: None,
            updated_at_ms: None,
        },
    }
}

fn read_project_document(path: &Path) -> anyhow::Result<Option<ProjectFileState>> {
    if !path.exists() {
        return Ok(None);
    }

    let content_md = fs::read_to_string(path)
        .with_context(|| format!("读取项目文档失败：{}", path.display()))?;
    let updated_at_ms = path_modified_ms(path).unwrap_or(0);

    Ok(Some(ProjectFileState {
        content_md,
        created_at_ms: updated_at_ms,
        updated_at_ms,
    }))
}

fn resolve_project_root(tool: CliToolId, project_id: &str) -> anyhow::Result<PathBuf> {
    let requested = canonical_project_root(Path::new(project_id)).ok_or_else(|| {
        StorageError::ProjectNotFound {
            project_id: project_id.to_string(),
        }
    })?;

    let requested_key = requested.to_string_lossy().to_string();
    let known_projects = discover_projects(tool)?;
    if known_projects
        .iter()
        .any(|item| item.root.to_string_lossy() == requested_key)
    {
        return Ok(requested);
    }

    Err(StorageError::ProjectNotFound {
        project_id: project_id.to_string(),
    }
    .into())
}

fn resolve_project_document_path(
    tool: CliToolId,
    scope: ProjectScope,
    project_id: Option<String>,
) -> anyhow::Result<(PathBuf, Option<String>)> {
    match scope {
        ProjectScope::Global => {
            anyhow::ensure!(project_id.is_none(), "全局项目文档不允许携带 project_id");
            Ok((global_project_document_path(tool)?, None))
        }
        ProjectScope::Project => {
            let project_id = project_id
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .ok_or_else(|| anyhow::anyhow!("项目文档缺少 project_id"))?;
            let root = resolve_project_root(tool, &project_id)?;
            let project_id = root.to_string_lossy().to_string();
            Ok((
                root.join(tool_project_document_filename(tool)),
                Some(project_id),
            ))
        }
    }
}

async fn run_project_io<T, F>(f: F) -> anyhow::Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> anyhow::Result<T> + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .context("等待项目文档任务失败")?
}

pub async fn list_projects(
    _db_path: PathBuf,
    tool: CliToolId,
) -> anyhow::Result<Vec<ProjectRecord>> {
    run_project_io(move || {
        let mut items = discover_projects(tool)?
            .into_iter()
            .map(|item| ProjectRecord {
                id: item.root.to_string_lossy().to_string(),
                name: default_project_name(&item.root),
                path: item.root.to_string_lossy().to_string(),
                created_at_ms: item.updated_at_ms,
                updated_at_ms: item.updated_at_ms,
            })
            .collect::<Vec<_>>();

        sort_projects(&mut items);
        Ok(items)
    })
    .await
}

pub async fn get_project_document(
    _db_path: PathBuf,
    tool: CliToolId,
    scope: ProjectScope,
    project_id: Option<String>,
) -> anyhow::Result<ProjectDocument> {
    run_project_io(move || {
        let (path, project_id) = resolve_project_document_path(tool, scope, project_id)?;
        let state = read_project_document(&path)?;
        Ok(project_document_from_file(tool, scope, project_id, state))
    })
    .await
}

pub async fn save_project_document(
    _db_path: PathBuf,
    input: SaveProjectDocument,
) -> anyhow::Result<ProjectDocument> {
    run_project_io(move || {
        validate_document_size(&input.content_md)?;

        let (path, project_id) =
            resolve_project_document_path(input.tool, input.scope, input.project_id)?;
        let existing = read_project_document(&path)?;
        ensure_document_version(
            existing.as_ref().map(|item| item.updated_at_ms),
            input.expected_updated_at_ms,
        )?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("创建项目文档目录失败：{}", parent.display()))?;
        }

        fs::write(&path, input.content_md.as_bytes())
            .with_context(|| format!("保存项目文档失败：{}", path.display()))?;

        let saved = read_project_document(&path)?.context("项目文档保存后读取文件失败")?;
        Ok(project_document_from_file(
            input.tool,
            input.scope,
            project_id,
            Some(saved),
        ))
    })
    .await
}

pub async fn delete_project_document(
    _db_path: PathBuf,
    input: DeleteProjectDocument,
) -> anyhow::Result<()> {
    run_project_io(move || {
        let (path, _project_id) =
            resolve_project_document_path(input.tool, input.scope, input.project_id)?;
        let existing =
            read_project_document(&path)?.ok_or(StorageError::ProjectDocumentNotFound)?;
        ensure_document_version(Some(existing.updated_at_ms), input.expected_updated_at_ms)?;

        fs::remove_file(&path).with_context(|| format!("删除项目文档失败：{}", path.display()))?;
        Ok(())
    })
    .await
}

pub async fn delete_project(
    _db_path: PathBuf,
    tool: CliToolId,
    project_id: String,
) -> anyhow::Result<()> {
    run_project_io(move || {
        let target_root = resolve_project_root(tool, &project_id)?;

        match tool {
            CliToolId::Claude => {
                let session_files = collect_session_files_for_project(
                    &claude_home_dir()?.join("projects"),
                    &target_root,
                )?;
                remove_files(&session_files, "Claude session 文件")?;
            }
            CliToolId::Codex => {
                let session_files = collect_session_files_for_project(
                    &codex_home_dir()?.join("sessions"),
                    &target_root,
                )?;
                remove_files(&session_files, "Codex session 文件")?;
            }
            CliToolId::Gemini => {
                remove_gemini_project_entries(&gemini_home_dir()?, &target_root)?;
            }
        }

        Ok(())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::sync::{Mutex, OnceLock};

    fn temp_dir(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("cliswitch-prompt-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn cleanup_dir(path: &Path) {
        let _ = fs::remove_dir_all(path);
    }

    fn run_async_test<F>(f: F)
    where
        F: std::future::Future<Output = ()>,
    {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(f);
    }

    fn env_lock() -> &'static Mutex<()> {
        static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        ENV_LOCK.get_or_init(|| Mutex::new(()))
    }

    struct ScopedEnvVar {
        _guard: std::sync::MutexGuard<'static, ()>,
        key: &'static str,
        previous: Option<OsString>,
    }

    impl ScopedEnvVar {
        fn set_path(key: &'static str, value: &Path) -> Self {
            let guard = env_lock().lock().unwrap();
            let previous = std::env::var_os(key);
            unsafe {
                std::env::set_var(key, value);
            }
            Self {
                _guard: guard,
                key,
                previous,
            }
        }
    }

    impl Drop for ScopedEnvVar {
        fn drop(&mut self) {
            match self.previous.as_ref() {
                Some(previous) => unsafe {
                    std::env::set_var(self.key, previous);
                },
                None => unsafe {
                    std::env::remove_var(self.key);
                },
            }
        }
    }

    #[test]
    fn extract_cwd_from_jsonl_reads_top_level_field() {
        let dir = temp_dir("extract-cwd");
        let file = dir.join("session.jsonl");
        fs::write(
            &file,
            r#"{"type":"session_meta","cwd":"/tmp/project-a"}
{"type":"response_item"}
"#,
        )
        .unwrap();

        let cwd = extract_cwd_from_jsonl(&file);
        assert_eq!(cwd.as_deref(), Some("/tmp/project-a"));
        cleanup_dir(&dir);
    }

    #[test]
    fn discover_codex_projects_uses_git_root_and_deduplicates() {
        let dir = temp_dir("codex-discovery");
        let repo_root = dir.join("repo");
        let nested = repo_root.join("packages/app");
        let sessions_root = dir.join("sessions/2026/03/08");

        fs::create_dir_all(&nested).unwrap();
        fs::create_dir_all(&sessions_root).unwrap();
        fs::write(repo_root.join(".git"), "gitdir: /tmp/worktree").unwrap();
        fs::write(
            sessions_root.join("a.jsonl"),
            format!(
                "{{\"type\":\"session_meta\",\"cwd\":\"{}\"}}\n",
                nested.display()
            ),
        )
        .unwrap();
        fs::write(
            sessions_root.join("b.jsonl"),
            format!(
                "{{\"type\":\"session_meta\",\"cwd\":\"{}\"}}\n",
                repo_root.display()
            ),
        )
        .unwrap();

        let projects = discover_codex_projects_in_dir(&dir.join("sessions")).unwrap();
        let expected_root = fs::canonicalize(&repo_root).unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].root, expected_root);
        cleanup_dir(&dir);
    }

    #[test]
    fn discover_claude_projects_uses_git_root_and_deduplicates() {
        let dir = temp_dir("claude-discovery");
        let repo_root = dir.join("repo");
        let nested = repo_root.join("packages/app");
        let projects_root = dir.join("projects/workspace");

        fs::create_dir_all(&nested).unwrap();
        fs::create_dir_all(&projects_root).unwrap();
        fs::write(repo_root.join(".git"), "gitdir: /tmp/worktree").unwrap();
        fs::write(
            projects_root.join("a.jsonl"),
            format!(
                "{{\"type\":\"session_meta\",\"cwd\":\"{}\"}}\n",
                nested.display()
            ),
        )
        .unwrap();
        fs::write(
            projects_root.join("b.jsonl"),
            format!(
                "{{\"type\":\"session_meta\",\"payload\":{{\"cwd\":\"{}\"}}}}\n",
                repo_root.display()
            ),
        )
        .unwrap();

        let projects = discover_claude_projects_in_dir(&dir.join("projects")).unwrap();
        let expected_root = fs::canonicalize(&repo_root).unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].root, expected_root);
        cleanup_dir(&dir);
    }

    #[test]
    fn discover_gemini_projects_reads_projects_json() {
        let dir = temp_dir("gemini-discovery");
        let gemini_root = dir.join(".gemini");
        let project_root = dir.join("repo");

        fs::create_dir_all(&gemini_root).unwrap();
        fs::create_dir_all(&project_root).unwrap();
        fs::write(project_root.join(".git"), "gitdir: /tmp/worktree").unwrap();
        fs::write(
            gemini_root.join("projects.json"),
            format!(
                "{{\"projects\":{{\"{}\":\"repo\",\"/\":\"root\"}}}}",
                project_root.display()
            ),
        )
        .unwrap();

        let projects = discover_gemini_projects_in_dir(&gemini_root).unwrap();
        let expected_root = fs::canonicalize(&project_root).unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].root, expected_root);
        cleanup_dir(&dir);
    }

    #[test]
    fn save_project_document_rejects_oversized_content() {
        let dir = temp_dir("too-large");

        {
            let _env = ScopedEnvVar::set_path("CODEX_HOME", &dir);
            run_async_test(async {
                let err = save_project_document(
                    PathBuf::new(),
                    SaveProjectDocument {
                        tool: CliToolId::Codex,
                        scope: ProjectScope::Global,
                        project_id: None,
                        content_md: "a".repeat(PROJECT_DOCUMENT_MAX_BYTES + 1),
                        expected_updated_at_ms: None,
                    },
                )
                .await
                .unwrap_err();

                assert!(matches!(
                    err.downcast_ref::<StorageError>(),
                    Some(StorageError::ProjectDocumentTooLarge { .. })
                ));
            });
        }

        cleanup_dir(&dir);
    }

    #[test]
    fn project_document_uses_optimistic_lock_and_can_delete() {
        let dir = temp_dir("optimistic-lock");

        {
            let _env = ScopedEnvVar::set_path("CODEX_HOME", &dir);
            run_async_test(async {
                let first = save_project_document(
                    PathBuf::new(),
                    SaveProjectDocument {
                        tool: CliToolId::Codex,
                        scope: ProjectScope::Global,
                        project_id: None,
                        content_md: "hello".to_string(),
                        expected_updated_at_ms: None,
                    },
                )
                .await
                .unwrap();

                // Give the filesystem mtime enough room to advance before the next write.
                std::thread::sleep(std::time::Duration::from_millis(10));

                let second = save_project_document(
                    PathBuf::new(),
                    SaveProjectDocument {
                        tool: CliToolId::Codex,
                        scope: ProjectScope::Global,
                        project_id: None,
                        content_md: "world".to_string(),
                        expected_updated_at_ms: first.updated_at_ms,
                    },
                )
                .await
                .unwrap();

                assert_ne!(first.updated_at_ms, second.updated_at_ms);

                let err = save_project_document(
                    PathBuf::new(),
                    SaveProjectDocument {
                        tool: CliToolId::Codex,
                        scope: ProjectScope::Global,
                        project_id: None,
                        content_md: "stale".to_string(),
                        expected_updated_at_ms: first.updated_at_ms,
                    },
                )
                .await
                .unwrap_err();

                assert!(matches!(
                    err.downcast_ref::<StorageError>(),
                    Some(StorageError::ProjectDocumentVersionConflict { .. })
                ));

                delete_project_document(
                    PathBuf::new(),
                    DeleteProjectDocument {
                        tool: CliToolId::Codex,
                        scope: ProjectScope::Global,
                        project_id: None,
                        expected_updated_at_ms: second.updated_at_ms,
                    },
                )
                .await
                .unwrap();

                let doc = get_project_document(
                    PathBuf::new(),
                    CliToolId::Codex,
                    ProjectScope::Global,
                    None,
                )
                .await
                .unwrap();
                assert!(!doc.exists);
                assert!(doc.content_md.is_empty());
            });
        }

        cleanup_dir(&dir);
    }

    #[test]
    fn delete_project_removes_codex_sessions_only() {
        let dir = temp_dir("delete-codex-project");
        let repo_root = dir.join("repo");
        let nested = repo_root.join("packages/app");
        let other_root = dir.join("other-repo");
        let sessions_root = dir.join("sessions/2026/03/08");

        fs::create_dir_all(&nested).unwrap();
        fs::create_dir_all(&other_root).unwrap();
        fs::create_dir_all(&sessions_root).unwrap();
        fs::write(repo_root.join(".git"), "gitdir: /tmp/worktree").unwrap();
        fs::write(other_root.join(".git"), "gitdir: /tmp/worktree").unwrap();
        fs::write(repo_root.join("AGENTS.md"), "# prompt").unwrap();

        let target_session = sessions_root.join("target.jsonl");
        let other_session = sessions_root.join("other.jsonl");
        fs::write(
            &target_session,
            format!(
                "{{\"type\":\"session_meta\",\"cwd\":\"{}\"}}\n",
                nested.display()
            ),
        )
        .unwrap();
        fs::write(
            &other_session,
            format!(
                "{{\"type\":\"session_meta\",\"cwd\":\"{}\"}}\n",
                other_root.display()
            ),
        )
        .unwrap();

        {
            let _env = ScopedEnvVar::set_path("CODEX_HOME", &dir);
            run_async_test(async {
                let project_id = fs::canonicalize(&repo_root)
                    .unwrap()
                    .to_string_lossy()
                    .to_string();

                delete_project(PathBuf::new(), CliToolId::Codex, project_id)
                    .await
                    .unwrap();

                let projects = list_projects(PathBuf::new(), CliToolId::Codex)
                    .await
                    .unwrap();
                assert_eq!(projects.len(), 1);
                assert_eq!(
                    projects[0].path,
                    fs::canonicalize(&other_root).unwrap().to_string_lossy()
                );
                assert!(!target_session.exists());
                assert!(other_session.exists());
                assert!(repo_root.exists());
                assert!(repo_root.join("AGENTS.md").exists());
            });
        }

        cleanup_dir(&dir);
    }

    #[test]
    fn delete_project_removes_claude_sessions_only() {
        let dir = temp_dir("delete-claude-project");
        let repo_root = dir.join("repo");
        let nested = repo_root.join("packages/app");
        let other_root = dir.join("other-repo");
        let projects_root = dir.join("projects/workspace");

        fs::create_dir_all(&nested).unwrap();
        fs::create_dir_all(&other_root).unwrap();
        fs::create_dir_all(&projects_root).unwrap();
        fs::write(repo_root.join(".git"), "gitdir: /tmp/worktree").unwrap();
        fs::write(other_root.join(".git"), "gitdir: /tmp/worktree").unwrap();
        fs::write(repo_root.join("CLAUDE.md"), "# prompt").unwrap();

        let target_session = projects_root.join("target.jsonl");
        let other_session = projects_root.join("other.jsonl");
        fs::write(
            &target_session,
            format!(
                "{{\"type\":\"session_meta\",\"cwd\":\"{}\"}}\n",
                nested.display()
            ),
        )
        .unwrap();
        fs::write(
            &other_session,
            format!(
                "{{\"type\":\"session_meta\",\"payload\":{{\"cwd\":\"{}\"}}}}\n",
                other_root.display()
            ),
        )
        .unwrap();

        {
            let _env = ScopedEnvVar::set_path("CLAUDE_CONFIG_DIR", &dir);
            run_async_test(async {
                let project_id = fs::canonicalize(&repo_root)
                    .unwrap()
                    .to_string_lossy()
                    .to_string();

                delete_project(PathBuf::new(), CliToolId::Claude, project_id)
                    .await
                    .unwrap();

                let projects = list_projects(PathBuf::new(), CliToolId::Claude)
                    .await
                    .unwrap();
                assert_eq!(projects.len(), 1);
                assert_eq!(
                    projects[0].path,
                    fs::canonicalize(&other_root).unwrap().to_string_lossy()
                );
                assert!(!target_session.exists());
                assert!(other_session.exists());
                assert!(repo_root.exists());
                assert!(repo_root.join("CLAUDE.md").exists());
            });
        }

        cleanup_dir(&dir);
    }

    #[test]
    fn delete_project_removes_gemini_discovery_entries() {
        let dir = temp_dir("delete-gemini-project");
        let home = dir.join("home");
        let gemini_root = home.join(".gemini");
        let repo_root = dir.join("repo");
        let other_root = dir.join("other-repo");
        let history_root = gemini_root.join("history/a");
        let tmp_root = gemini_root.join("tmp/b");

        fs::create_dir_all(&history_root).unwrap();
        fs::create_dir_all(&tmp_root).unwrap();
        fs::create_dir_all(&repo_root).unwrap();
        fs::create_dir_all(&other_root).unwrap();
        fs::write(repo_root.join(".git"), "gitdir: /tmp/worktree").unwrap();
        fs::write(other_root.join(".git"), "gitdir: /tmp/worktree").unwrap();
        fs::write(
            gemini_root.join("projects.json"),
            format!(
                "{{\"projects\":{{\"{}\":\"repo\",\"{}\":\"other\"}}}}",
                repo_root.display(),
                other_root.display()
            ),
        )
        .unwrap();
        fs::write(
            history_root.join(".project_root"),
            repo_root.to_string_lossy().as_ref(),
        )
        .unwrap();
        fs::write(
            tmp_root.join(".project_root"),
            other_root.to_string_lossy().as_ref(),
        )
        .unwrap();

        {
            let _env = ScopedEnvVar::set_path("HOME", &home);
            run_async_test(async {
                let project_id = fs::canonicalize(&repo_root)
                    .unwrap()
                    .to_string_lossy()
                    .to_string();

                delete_project(PathBuf::new(), CliToolId::Gemini, project_id)
                    .await
                    .unwrap();

                let projects = list_projects(PathBuf::new(), CliToolId::Gemini)
                    .await
                    .unwrap();
                assert_eq!(projects.len(), 1);
                assert_eq!(
                    projects[0].path,
                    fs::canonicalize(&other_root).unwrap().to_string_lossy()
                );
                assert!(!history_root.join(".project_root").exists());
                assert!(tmp_root.join(".project_root").exists());
                assert!(repo_root.exists());
            });
        }

        cleanup_dir(&dir);
    }
}
