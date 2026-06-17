import { describe, expect, it } from "vitest";
import { isTauriDesktopShell } from "./shell-platform";

describe("shell-platform", () => {
  it("detects tauri globals on the window object", () => {
    expect(isTauriDesktopShell({} as Window)).toBe(false);
    expect(
      isTauriDesktopShell({ __TAURI_INTERNALS__: {} } as Window & { __TAURI_INTERNALS__: object }),
    ).toBe(true);
    expect(isTauriDesktopShell({ __TAURI__: {} } as Window & { __TAURI__: object })).toBe(true);
  });
});
