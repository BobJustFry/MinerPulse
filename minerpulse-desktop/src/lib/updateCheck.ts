import { check, type Update } from "@tauri-apps/plugin-updater";

export const UPDATE_CHECK_INTERVAL_MS = 15 * 60 * 1000;

export type UpdateCheckResult =
  | { status: "available"; versionLabel: string }
  | { status: "up_to_date" }
  | { status: "unavailable" };

export function formatUpdateVersion(update: Update, productVersion: string): string {
  const build = update.rawJson?.["build"];
  if (typeof build === "number") {
    return `${productVersion} (${build})`;
  }
  return update.version;
}

export async function checkForAppUpdate(productVersion: string): Promise<UpdateCheckResult> {
  try {
    const update = await check();
    if (update) {
      await update.close();
      return {
        status: "available",
        versionLabel: formatUpdateVersion(update, productVersion),
      };
    }
    return { status: "up_to_date" };
  } catch {
    return { status: "unavailable" };
  }
}
