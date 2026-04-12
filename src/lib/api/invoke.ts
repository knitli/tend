// T029: typed wrapper around `@tauri-apps/api/core::invoke` with structured
// error conversion. Every Tauri command returns JSON that matches the
// `WorkbenchError` shape from `contracts/tauri-commands.md §8` on the error
// path; we rethrow as a real `WorkbenchError` class so the UI can branch on
// `.code` without string-matching.

import { invoke as tauriInvoke } from "@tauri-apps/api/core";

/**
 * Machine-readable error codes mirrored from `src-tauri/src/error.rs`.
 */
export type WorkbenchErrorCode =
  | "NOT_FOUND"
  | "ALREADY_EXISTS"
  | "ALREADY_REGISTERED"
  | "NOT_ARCHIVED"
  | "PATH_NOT_FOUND"
  | "PATH_NOT_A_DIRECTORY"
  | "WORKING_DIRECTORY_INVALID"
  | "PROJECT_ARCHIVED"
  | "SPAWN_FAILED"
  | "COMPANION_SPAWN_FAILED"
  | "WRITE_FAILED"
  | "SESSION_ENDED"
  | "SESSION_READ_ONLY"
  | "CONTENT_EMPTY"
  | "NAME_TAKEN"
  | "PROTOCOL_ERROR"
  | "MESSAGE_TOO_LARGE"
  | "UNAUTHORIZED"
  | "INTERNAL";

/**
 * Strongly-typed error raised by `invoke()` on command failure.
 *
 * The backend returns `{ code, message, details? }` as a JSON payload in
 * the Tauri invoke reject path; we parse that here and re-throw as a real
 * Error subclass so call sites can do `err instanceof WorkbenchError`.
 */
export class WorkbenchError extends Error {
  readonly code: WorkbenchErrorCode;
  readonly details: Record<string, unknown> | undefined;

  constructor(
    code: WorkbenchErrorCode,
    message: string,
    details?: Record<string, unknown>,
  ) {
    super(`[${code}] ${message}`);
    this.name = "WorkbenchError";
    this.code = code;
    this.details = details;
  }
}

function isWorkbenchErrorShape(
  value: unknown,
): value is { code: WorkbenchErrorCode; message: string; details?: Record<string, unknown> } {
  if (typeof value !== "object" || value === null) return false;
  const obj = value as Record<string, unknown>;
  return typeof obj.code === "string" && typeof obj.message === "string";
}

/**
 * Thin typed wrapper around the Tauri `invoke` function.
 *
 * Usage:
 *   const { projects } = await invoke<{ projects: Project[] }>("project_list", { includeArchived: true });
 */
export async function invoke<R>(
  command: string,
  args?: Record<string, unknown>,
): Promise<R> {
  try {
    return await tauriInvoke<R>(command, args);
  } catch (err) {
    if (isWorkbenchErrorShape(err)) {
      throw new WorkbenchError(err.code, err.message, err.details);
    }
    // Fallback for unexpected reject shapes (Tauri internal errors, etc.).
    throw new WorkbenchError(
      "INTERNAL",
      typeof err === "string" ? err : `${err}`,
    );
  }
}
