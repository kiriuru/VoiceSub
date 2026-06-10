import { describe, expect, it } from "vitest";

import { setLocale, t } from "./index";

describe("style slot i18n", () => {
  it("serves apply-preset labels in en and ru", () => {
    setLocale("en");
    expect(t("style.slots.apply_preset")).toBe("Apply preset to this slot");

    setLocale("ru");
    expect(t("style.slots.apply_preset")).toBe("Применить пресет к этому слоту");
  });
});
