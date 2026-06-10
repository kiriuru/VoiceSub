export { applyUiThemeFromConfig, bootstrapTtsFromSettings } from "./app-settings";

/** @deprecated Use bootstrapTtsFromSettings */
export async function bootstrapTtsTheme(): Promise<void> {
  const { bootstrapTtsFromSettings } = await import("./app-settings");
  await bootstrapTtsFromSettings();
}
