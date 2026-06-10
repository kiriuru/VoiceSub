import { beforeEach, describe, expect, it } from "vitest";

import { partialOnlyPayload, renderer } from "./helpers";

function appendPreviewNote(container: HTMLElement, text: string): void {
  container.querySelectorAll(".subtitle-stage-note").forEach((node) => node.remove());
  const note = document.createElement("p");
  note.className = "subtitle-stage-note";
  note.textContent = text;
  container.appendChild(note);
}

describe("dashboard preview note cleanup", () => {
  let container: HTMLDivElement;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
  });

  it("does not accumulate subtitle-stage-note siblings across frames", () => {
    renderer().render(container, partialOnlyPayload("Hello"), { overlay: false });
    appendPreviewNote(container, "Live block #1");
    appendPreviewNote(container, "Live block #2");
    appendPreviewNote(container, "Live block #3");

    const notes = container.querySelectorAll(".subtitle-stage-note");
    expect(notes.length).toBe(1);
    expect(notes[0]?.textContent).toBe("Live block #3");
    expect(container.querySelector(".subtitle-stage-shell")).toBeTruthy();
  });
});
