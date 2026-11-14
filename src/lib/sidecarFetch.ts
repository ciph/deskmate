import { invoke, isTauri } from "@tauri-apps/api/core";
import { residentFetch } from "../../scripts/workbench-overlay/workbench/residency";
export function sidecarFetch(input: RequestInfo | URL, init?: RequestInit): Promise<Response> {
  return isTauri() ? residentFetch(invoke, input, init) : fetch(input, init);
}
