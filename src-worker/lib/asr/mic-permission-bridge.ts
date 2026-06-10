import type { AsrManagerHost } from "./types";

export function ensureMicrophonePermission(manager: AsrManagerHost): Promise<unknown> {
  if (manager._permissionPromise) {
    return manager._permissionPromise;
  }
  manager.appendLogInternal("requesting microphone permission");
  manager._permissionPromise = Promise.resolve(manager.options.ensureMicrophonePermission?.())
    .then((result) => {
      manager._permissionPromise = null;
      manager.state.getUserMediaLastError = null;
      manager.appendLogInternal("microphone permission granted");
      return result;
    })
    .catch((error: unknown) => {
      manager._permissionPromise = null;
      manager.state.getUserMediaLastError = error instanceof Error ? error.message : String(error || "");
      manager.appendLogInternal(
        `microphone permission failed: ${error instanceof Error ? error.message : error}`
      );
      throw error;
    });
  return manager._permissionPromise;
}
