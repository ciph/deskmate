import { invoke } from "@tauri-apps/api/core";

export interface WindowsComponentStatus {
  readonly phase: "unchecked" | "missing" | "downloading" | "verifying" | "cancelling" | "ready" | "error";
  readonly downloaded: number;
  readonly total: number;
  readonly version: string;
  readonly supported: boolean;
  readonly activation: "idle" | "waiting" | "applying" | "error";
  readonly error: string | null;
}
export const windowsComponentStatus = () => invoke<WindowsComponentStatus>("windows_component_status");
export const installWindowsComponent = () => invoke<void>("windows_component_install");
export const cancelWindowsComponent = () => invoke<void>("windows_component_cancel");
export const setWindowsComponentEnabled = (enabled: boolean) => invoke<void>("windows_component_set_enabled", { enabled });
export const retryWindowsComponentActivation = () => invoke<void>("windows_component_retry_activation");
