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
  // Allow backend to provide extra context for system package manager installs or errors.
  if (msg) return msg;

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

  return null;
}
