use anyhow::Context as _;
use semver::Version;
use std::time::Duration;

use super::{
    CliToolDef, CliToolId, CliToolInstallMethod, NPM_REGISTRY_NPMMIRROR, NPM_REGISTRY_OFFICIAL,
    normalize_version_string,
};

const VERSION_QUERY_TIMEOUT: Duration = Duration::from_secs(10);

struct VersionSources<'a> {
    npm_official: &'a str,
    npm_mirror: &'a str,
    brew_api: &'a str,
}

const VERSION_SOURCES: VersionSources<'static> = VersionSources {
    npm_official: NPM_REGISTRY_OFFICIAL,
    npm_mirror: NPM_REGISTRY_NPMMIRROR,
    brew_api: "https://formulae.brew.sh/api",
};

/// A release and the npm registry that supplied its version metadata, if applicable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LatestRelease {
    pub version: String,
    pub npm_registry: Option<String>,
}

/// Query release metadata for the tool's installer without running any commands.
pub async fn latest_version(
    client: &reqwest::Client,
    def: &CliToolDef,
    method: CliToolInstallMethod,
    npm_registry: &str,
) -> anyhow::Result<String> {
    Ok(latest_release(client, def, method, npm_registry)
        .await?
        .version)
}

/// Query a release once and retain its successful source for a subsequent pinned install.
pub async fn latest_release(
    client: &reqwest::Client,
    def: &CliToolDef,
    method: CliToolInstallMethod,
    npm_registry: &str,
) -> anyhow::Result<LatestRelease> {
    latest_release_with_sources(client, def, method, npm_registry, &VERSION_SOURCES).await
}

async fn latest_release_with_sources(
    client: &reqwest::Client,
    def: &CliToolDef,
    method: CliToolInstallMethod,
    npm_registry: &str,
    sources: &VersionSources<'_>,
) -> anyhow::Result<LatestRelease> {
    match method {
        CliToolInstallMethod::Brew => {
            let (path, field) = match def.id {
                CliToolId::Gemini => ("formula/gemini-cli.json", "/versions/stable"),
                CliToolId::Codex => ("cask/codex.json", "/version"),
                CliToolId::Claude => {
                    anyhow::bail!("Homebrew version queries are not supported for Claude Code")
                }
            };
            let url = format!("{}/{path}", sources.brew_api.trim_end_matches('/'));
            Ok(LatestRelease {
                version: fetch_version(client, &url, field).await?,
                npm_registry: None,
            })
        }
        CliToolInstallMethod::Npm
        | CliToolInstallMethod::ManagedNpmPrefix
        | CliToolInstallMethod::Other => {
            let registry = npm_registry.trim().trim_end_matches('/');
            let fallback = if registry == sources.npm_official.trim_end_matches('/') {
                sources.npm_mirror
            } else {
                // The mirror (or a custom registry) falls back to the official registry.
                sources.npm_official
            };
            let url = format!("{registry}/{}/latest", def.npm_package);
            let primary_error = match fetch_version(client, &url, "/version").await {
                Ok(version) => {
                    return Ok(LatestRelease {
                        version,
                        npm_registry: Some(registry.to_string()),
                    });
                }
                Err(error) => error,
            };
            let fallback = fallback.trim_end_matches('/');
            let fallback_url = format!("{fallback}/{}/latest", def.npm_package);
            let version = fetch_version(client, &fallback_url, "/version")
                .await
                .with_context(|| {
                    format!(
                        "preferred npm registry query {url} failed: {primary_error:#}; \
                         fallback query {fallback_url} also failed"
                    )
                })?;
            Ok(LatestRelease {
                version,
                npm_registry: Some(fallback.to_string()),
            })
        }
    }
}

async fn fetch_version(client: &reqwest::Client, url: &str, field: &str) -> anyhow::Result<String> {
    let metadata: serde_json::Value = client
        .get(url)
        // This bounds the entire request, including redirects and response body reads.
        .timeout(VERSION_QUERY_TIMEOUT)
        .send()
        .await
        .with_context(|| format!("request version metadata from {url}"))?
        .error_for_status()
        .with_context(|| format!("version metadata HTTP error from {url}"))?
        .json()
        .await
        .with_context(|| format!("invalid version metadata JSON from {url}"))?;
    let raw = metadata
        .pointer(field)
        .and_then(serde_json::Value::as_str)
        .with_context(|| format!("missing or non-string version field {field} from {url}"))?;
    // Metadata must contain a complete semver, rather than CLI output or a version range.
    let version = Version::parse(raw)
        .with_context(|| format!("invalid semantic version {raw:?} at {field} from {url}"))?;
    Ok(version.to_string())
}

/// Compare CLI version output using semver precedence, ignoring build metadata.
/// Invalid versions are errors, and a newer installed version is never downgraded.
pub fn is_update_available(current: &str, latest: &str) -> anyhow::Result<bool> {
    let current = Version::parse(&normalize_version_string(current))
        .with_context(|| format!("invalid current CLI version: {current:?}"))?;
    let latest = Version::parse(&normalize_version_string(latest))
        .with_context(|| format!("invalid latest CLI version: {latest:?}"))?;
    Ok(current.cmp_precedence(&latest).is_lt())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli_tools::CLI_TOOLS;
    use axum::Router;
    use axum::http::{StatusCode, Uri};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    #[test]
    fn equal_or_newer_current_versions_do_not_update() {
        for (current, latest) in [
            ("1.2.3", "1.2.3"),
            ("2.0.0", "1.9.9"),
            ("1.10.0", "1.9.0"),
            ("1.2.4", "1.2.3"),
        ] {
            assert!(!is_update_available(current, latest).unwrap());
        }
    }

    #[test]
    fn higher_major_minor_and_patch_versions_update() {
        for (current, latest) in [("1.9.9", "2.0.0"), ("1.9.0", "1.10.0"), ("1.2.9", "1.2.10")] {
            assert!(is_update_available(current, latest).unwrap());
        }
    }

    #[test]
    fn prerelease_versions_follow_semver_precedence() {
        for (current, latest, expected) in [
            ("1.2.3-alpha.2", "1.2.3-alpha.10", true),
            ("1.2.3-beta", "1.2.3-rc.1", true),
            ("1.2.3-rc.1", "1.2.3", true),
            ("1.2.3-rc.1", "1.2.3-rc.1", false),
            ("1.2.3", "1.2.3-rc.1", false),
            ("2.0.0-alpha", "1.9.9", false),
        ] {
            assert_eq!(is_update_available(current, latest).unwrap(), expected);
        }
    }

    #[test]
    fn normalizes_v_prefixes_and_cli_labels_without_losing_prereleases() {
        for (current, latest, expected) in [
            (" v1.2.3\n", "1.2.3", false),
            ("codex-cli 1.2.3", "v1.2.4", true),
            ("1.2.3 (Claude Code)", "1.2.4 (Claude Code)", true),
            ("Gemini CLI v1.2.3-rc.1", "1.2.3", true),
            ("1.2.3", "codex-cli v1.2.3-rc.1", false),
        ] {
            assert_eq!(is_update_available(current, latest).unwrap(), expected);
        }
    }

    #[test]
    fn build_metadata_does_not_change_version_precedence() {
        for (current, latest, expected) in [
            ("1.2.3+build.1", "1.2.3+build.2", false),
            ("1.2.3", "1.2.3+build.2", false),
            ("1.2.3+build.2", "1.2.3", false),
            ("1.2.3-rc.1+old", "1.2.3-rc.1+new", false),
            ("1.2.3+zzz", "1.2.4+aaa", true),
        ] {
            assert_eq!(is_update_available(current, latest).unwrap(), expected);
        }
    }

    #[test]
    fn invalid_current_or_latest_versions_are_errors() {
        for invalid in [
            "",
            "unknown",
            "latest",
            "v1.2",
            "1.2.3.4",
            "01.2.3",
            "1.2.3-01",
            "1.2.3+",
            "codex-cli unknown",
        ] {
            assert!(
                is_update_available(invalid, "1.2.3").is_err(),
                "{invalid:?}"
            );
            assert!(
                is_update_available("1.2.3", invalid).is_err(),
                "{invalid:?}"
            );
            assert!(
                is_update_available(invalid, invalid).is_err(),
                "{invalid:?}"
            );
        }
    }

    struct MockServer {
        official: String,
        mirror: String,
        brew: String,
        requests: Arc<Mutex<Vec<String>>>,
        task: tokio::task::JoinHandle<()>,
    }

    impl MockServer {
        async fn start(routes: &[(&str, StatusCode, &str)]) -> Self {
            let routes: HashMap<_, _> = routes
                .iter()
                .map(|(path, status, body)| (path.to_string(), (*status, body.to_string())))
                .collect();
            let requests = Arc::new(Mutex::new(Vec::new()));
            let recorded = requests.clone();
            let app = Router::new().fallback(move |uri: Uri| {
                recorded.lock().unwrap().push(uri.path().to_string());
                let response = routes
                    .get(uri.path())
                    .cloned()
                    .unwrap_or((StatusCode::NOT_FOUND, "unexpected request".to_string()));
                async move { response }
            });
            let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
                .await
                .unwrap();
            let address = listener.local_addr().unwrap();
            let task = tokio::spawn(async move {
                axum::serve(listener, app).await.unwrap();
            });
            Self {
                official: format!("http://{address}/official"),
                mirror: format!("http://{address}/mirror"),
                brew: format!("http://{address}/api"),
                requests,
                task,
            }
        }

        fn sources(&self) -> VersionSources<'_> {
            VersionSources {
                npm_official: &self.official,
                npm_mirror: &self.mirror,
                brew_api: &self.brew,
            }
        }

        fn requests(&self) -> Vec<String> {
            self.requests.lock().unwrap().clone()
        }
    }

    impl Drop for MockServer {
        fn drop(&mut self) {
            self.task.abort();
        }
    }

    fn client() -> reqwest::Client {
        reqwest::Client::builder().no_proxy().build().unwrap()
    }

    fn tool(id: CliToolId) -> &'static CliToolDef {
        CLI_TOOLS.iter().find(|def| def.id == id).unwrap()
    }

    #[tokio::test]
    async fn latest_version_returns_a_string_with_one_request() {
        let server = MockServer::start(&[(
            "/official/@openai/codex/latest",
            StatusCode::OK,
            r#"{"version":"1.2.3"}"#,
        )])
        .await;
        let version: String = latest_version(
            &client(),
            tool(CliToolId::Codex),
            CliToolInstallMethod::Npm,
            &server.official,
        )
        .await
        .unwrap();
        assert_eq!(version, "1.2.3");
        assert_eq!(server.requests(), ["/official/@openai/codex/latest"]);
    }

    #[tokio::test]
    async fn npm_methods_query_each_packages_latest_metadata() {
        let server = MockServer::start(&[
            (
                "/official/@google/gemini-cli/latest",
                StatusCode::OK,
                r#"{"version":"1.2.3"}"#,
            ),
            (
                "/official/@anthropic-ai/claude-code/latest",
                StatusCode::OK,
                r#"{"version":"2.3.4"}"#,
            ),
            (
                "/official/@openai/codex/latest",
                StatusCode::OK,
                r#"{"version":"3.4.5-rc.1+build.7"}"#,
            ),
        ])
        .await;
        let client = client();
        for method in [
            CliToolInstallMethod::Npm,
            CliToolInstallMethod::ManagedNpmPrefix,
            CliToolInstallMethod::Other,
        ] {
            for (id, expected) in [
                (CliToolId::Gemini, "1.2.3"),
                (CliToolId::Claude, "2.3.4"),
                (CliToolId::Codex, "3.4.5-rc.1+build.7"),
            ] {
                let release = latest_release_with_sources(
                    &client,
                    tool(id),
                    method,
                    &format!("{}/", server.official),
                    &server.sources(),
                )
                .await
                .unwrap();
                assert_eq!(release.version, expected);
                assert_eq!(
                    release.npm_registry.as_deref(),
                    Some(server.official.as_str())
                );
            }
        }
        assert_eq!(server.requests().len(), 9);
        assert!(
            server
                .requests()
                .iter()
                .all(|p| p.starts_with("/official/"))
        );
    }

    #[tokio::test]
    async fn successful_selected_registry_does_not_query_fallback() {
        let server = MockServer::start(&[
            (
                "/mirror/@openai/codex/latest",
                StatusCode::OK,
                r#"{"version":"1.2.3"}"#,
            ),
            (
                "/official/@openai/codex/latest",
                StatusCode::OK,
                r#"{"version":"2.0.0"}"#,
            ),
        ])
        .await;
        let release = latest_release(
            &client(),
            tool(CliToolId::Codex),
            CliToolInstallMethod::Npm,
            &server.mirror,
        )
        .await
        .unwrap();
        assert_eq!(release.version, "1.2.3");
        assert_eq!(
            release.npm_registry.as_deref(),
            Some(server.mirror.as_str())
        );
        assert_eq!(server.requests(), ["/mirror/@openai/codex/latest"]);
    }

    #[tokio::test]
    async fn npm_http_json_and_version_errors_fall_back_in_both_directions() {
        for (status, body) in [
            (StatusCode::NOT_FOUND, r#"{"version":"9.9.9"}"#),
            (StatusCode::INTERNAL_SERVER_ERROR, "unavailable"),
            (StatusCode::OK, "not json"),
            (StatusCode::OK, "{}"),
            (StatusCode::OK, r#"{"version":null}"#),
            (StatusCode::OK, r#"{"version":123}"#),
            (StatusCode::OK, r#"{"version":"latest"}"#),
            (StatusCode::OK, r#"{"version":"1.2"}"#),
            (StatusCode::OK, r#"{"version":"v1.2.3"}"#),
            (StatusCode::OK, r#"{"version":"1.2.3 trailing"}"#),
            (StatusCode::OK, r#"{"version":" 1.2.3 "}"#),
            (StatusCode::OK, r#"{"version":"1.2.3-01"}"#),
        ] {
            for (primary, fallback) in [("official", "mirror"), ("mirror", "official")] {
                let primary_path = format!("/{primary}/@openai/codex/latest");
                let fallback_path = format!("/{fallback}/@openai/codex/latest");
                let server = MockServer::start(&[
                    (&primary_path, status, body),
                    (&fallback_path, StatusCode::OK, r#"{"version":"1.2.4"}"#),
                ])
                .await;
                let registry = if primary == "official" {
                    &server.official
                } else {
                    &server.mirror
                };
                let release = latest_release_with_sources(
                    &client(),
                    tool(CliToolId::Codex),
                    CliToolInstallMethod::Npm,
                    registry,
                    &server.sources(),
                )
                .await
                .unwrap();
                assert_eq!(release.version, "1.2.4", "{primary}: {status} {body}");
                let expected_registry = if fallback == "official" {
                    &server.official
                } else {
                    &server.mirror
                };
                assert_eq!(
                    release.npm_registry.as_deref(),
                    Some(expected_registry.as_str())
                );
                assert_eq!(server.requests(), [primary_path, fallback_path]);
            }
        }
    }

    #[tokio::test]
    async fn npm_reports_both_errors_when_fallback_also_fails() {
        let server = MockServer::start(&[
            (
                "/official/@openai/codex/latest",
                StatusCode::NOT_FOUND,
                "missing",
            ),
            (
                "/mirror/@openai/codex/latest",
                StatusCode::OK,
                r#"{"version":"invalid"}"#,
            ),
        ])
        .await;
        let error = latest_release_with_sources(
            &client(),
            tool(CliToolId::Codex),
            CliToolInstallMethod::Npm,
            &server.official,
            &server.sources(),
        )
        .await
        .unwrap_err();
        let message = format!("{error:#}");
        assert!(message.contains("404"), "{message}");
        assert!(message.contains("invalid semantic version"), "{message}");
        assert!(message.contains(&server.official), "{message}");
        assert!(message.contains(&server.mirror), "{message}");
        assert_eq!(server.requests().len(), 2);
    }

    #[tokio::test]
    async fn brew_uses_gemini_formula_stable_and_codex_cask_version() {
        let server = MockServer::start(&[
            (
                "/api/formula/gemini-cli.json",
                StatusCode::OK,
                r#"{"version":"9.9.9","versions":{"stable":"1.2.3","head":"HEAD"}}"#,
            ),
            (
                "/api/cask/codex.json",
                StatusCode::OK,
                r#"{"version":"2.3.4","versions":{"stable":"9.9.9"}}"#,
            ),
        ])
        .await;
        for (id, expected) in [(CliToolId::Gemini, "1.2.3"), (CliToolId::Codex, "2.3.4")] {
            let release = latest_release_with_sources(
                &client(),
                tool(id),
                CliToolInstallMethod::Brew,
                &server.official,
                &server.sources(),
            )
            .await
            .unwrap();
            assert_eq!(release.version, expected);
            assert_eq!(release.npm_registry, None);
        }
        assert_eq!(
            server.requests(),
            ["/api/formula/gemini-cli.json", "/api/cask/codex.json"]
        );
    }

    #[tokio::test]
    async fn brew_http_json_and_invalid_versions_are_errors() {
        for (id, path) in [
            (CliToolId::Gemini, "/api/formula/gemini-cli.json"),
            (CliToolId::Codex, "/api/cask/codex.json"),
        ] {
            for (status, body) in [
                (StatusCode::BAD_GATEWAY, "unavailable"),
                (StatusCode::OK, "not json"),
                (StatusCode::OK, "{}"),
                (
                    StatusCode::OK,
                    r#"{"version":null,"versions":{"stable":null}}"#,
                ),
                (
                    StatusCode::OK,
                    r#"{"version":"1.2","versions":{"stable":"1.2"}}"#,
                ),
                (
                    StatusCode::OK,
                    r#"{"version":"1.2.3,4","versions":{"stable":"HEAD"}}"#,
                ),
            ] {
                let server = MockServer::start(&[(path, status, body)]).await;
                let result = latest_release_with_sources(
                    &client(),
                    tool(id),
                    CliToolInstallMethod::Brew,
                    &server.official,
                    &server.sources(),
                )
                .await;
                assert!(result.is_err(), "{id:?}: {status} {body}");
                assert_eq!(server.requests(), [path]);
            }
        }
    }

    #[tokio::test]
    async fn brew_claude_is_unsupported_without_any_request() {
        let server = MockServer::start(&[]).await;
        let error = latest_version(
            &client(),
            tool(CliToolId::Claude),
            CliToolInstallMethod::Brew,
            &server.official,
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("not supported for Claude Code"));
        assert!(server.requests().is_empty());
    }
}
