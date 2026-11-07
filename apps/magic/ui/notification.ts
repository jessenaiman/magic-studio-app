// Notification helper — permission requested on enabling (Plugin 3).
// Mirrors Tauri plugin-notification docs: check -> request -> send.
import { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/plugin-notification";

export async function notificationPermission(): Promise<boolean> {
  return await isPermissionGranted();
}

export async function requestNotificationPermission(): Promise<boolean> {
  const result = await requestPermission();
  return result === "granted";
}

export async function notifyIfGranted(title: string, body: string): Promise<boolean> {
  if (!(await isPermissionGranted())) return false;
  await sendNotification({ title, body });
  return true;
}

// Long-task notifier for render/export/import finish/fail (called after executeCommand resolves).
export async function notifyTaskComplete(kind: string, ok: boolean, detail?: string): Promise<boolean> {
  const title = ok ? `${kind} finished` : `${kind} failed`;
  const body = detail?.slice(0, 240) || (ok ? "Done" : "See console for error");
  return notifyIfGranted(title, body);
}
