import type { Backend } from "./backend";
import { MockBackend } from "./mockBackend";
import { tauriBackend } from "./tauriBackend";

export const backend: Backend = "__TAURI_INTERNALS__" in window ? tauriBackend : new MockBackend();
