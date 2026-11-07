// Pure OS-info helpers — no Tauri imports so node-vm tests can load them.
// api.ts / desktop.ts supply values from @tauri-apps/plugin-os and delegate here.

export type OsInfoInput = {
  platform: string;
  family: string;
  arch: string;
  version: string;
  osType: string;
  exeExtension: string;
  eol: string;
  locale: string | null;
};

export type OsInfo = {
  platform: string;
  family: string;
  arch: string;
  version: string;
  osType: string;
  exeExtension: string;
  eol: string;
  locale: string | null;
};

const PLATFORMS = new Set(["linux", "macos", "ios", "freebsd", "dragonfly", "netbsd", "openbsd", "solaris", "android", "windows"]);
const FAMILIES = new Set(["unix", "windows"]);

function normalizePlatform(raw: string): string {
  const p = String(raw || "").trim().toLowerCase();
  if (p === "darwin") return "macos";
  if (!PLATFORMS.has(p)) throw new Error(`Unknown platform ${raw}`);
  return p;
}

export function formatOsInfo(input: OsInfoInput): OsInfo {
  const family = String(input.family || "").trim().toLowerCase();
  if (!FAMILIES.has(family)) throw new Error(`Unknown family ${input.family}`);
  const platform = normalizePlatform(input.platform);
  return {
    platform,
    family,
    arch: String(input.arch || ""),
    version: String(input.version || ""),
    osType: String(input.osType || ""),
    exeExtension: String(input.exeExtension || ""),
    eol: String(input.eol || "\n"),
    locale: input.locale ?? null,
  };
}

export function isMacOSPlatform(platform: string): boolean {
  return normalizePlatform(platform) === "macos";
}

export function isWindowsPlatform(platform: string): boolean {
  return normalizePlatform(platform) === "windows";
}

export function isUnixFamily(family: string): boolean {
  return String(family || "").trim().toLowerCase() === "unix";
}

export function modifierLabel(platform: string): string {
  return isMacOSPlatform(platform) ? "Cmd" : "Ctrl";
}
