import type { NpmEnvInstallProgress } from "@/lib/cliswitchEvents";

type TFunction = (key: string, vars?: Record<string, string | number>) => string;

export function formatNpmEnvInstallProgressText(
  t: TFunction,
  installing: boolean,
  progress: NpmEnvInstallProgress | null
): string | null {
  if (!installing) return null;
  if (!progress) return null;

  const msg = (progress.message ?? "").trim();
  // Prefer stage-based i18n. Only fall back to backend-provided message when we don't know the stage.
  // This avoids showing hard-coded English strings in localized UIs.
  if (progress.stage === "checking_system") return t("settings.cliTools.npmEnvProgressCheckingSystem");
  if (progress.stage === "system_install_winget") return t("settings.cliTools.npmEnvProgressSystemInstallWinget");
  if (progress.stage === "system_install_brew") return t("settings.cliTools.npmEnvProgressSystemInstallBrew");
  if (progress.stage === "system_install_apt_get") return t("settings.cliTools.npmEnvProgressSystemInstallAptGet");
  if (progress.stage === "system_install_dnf") return t("settings.cliTools.npmEnvProgressSystemInstallDnf");
  if (progress.stage === "system_install_yum") return t("settings.cliTools.npmEnvProgressSystemInstallYum");
  if (progress.stage === "system_install_failed") return t("settings.cliTools.npmEnvProgressSystemInstallFailed");

  if (progress.stage === "resolving_version") return t("settings.cliTools.npmEnvProgressResolving");
  if (progress.stage === "downloading_shasums") return t("settings.cliTools.npmEnvProgressDownloadingShasums");
  if (progress.stage === "downloading_archive") {
    return progress.percent !== null
      ? t("settings.cliTools.npmEnvProgressDownloadingArchivePercent", { percent: progress.percent })
      : t("settings.cliTools.npmEnvProgressDownloadingArchive");
  }
  if (progress.stage === "verifying_sha256") return t("settings.cliTools.npmEnvProgressVerifying");
  if (progress.stage === "extracting") return t("settings.cliTools.npmEnvProgressExtracting");
  if (progress.stage === "done") return t("settings.cliTools.npmEnvProgressDone");
  if (progress.stage === "error") return t("settings.cliTools.npmEnvProgressError");

  return msg || null;
}
