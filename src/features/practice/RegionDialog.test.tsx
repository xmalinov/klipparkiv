import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { REGION_COLORS } from "./PracticeProvider";
import RegionDialog, { type RegionDraft } from "./RegionDialog";

vi.mock("../../i18n", () => ({
  useI18n: () => ({
    language: "en",
    setLanguage: vi.fn(),
    t: (key: string) => {
      const messages: Record<string, string> = {
        "practice.addRegion": "Add region",
        "practice.regionName": "Name",
        "practice.regions": "Region",
        "common.close": "Close",
        "common.cancel": "Cancel",
        "common.save": "Save",
      };
      if (messages[key]) return messages[key];
      const tail = key.split(".");
      return tail[tail.length - 1];
    },
  }),
}));

function setup(initial?: RegionDraft) {
  const onSave = vi.fn();
  const onCancel = vi.fn();
  const view = render(
    <RegionDialog initial={initial} onCancel={onCancel} onSave={onSave} />,
  );
  const name = screen.getByLabelText("Name") as HTMLInputElement;
  const kind = document.getElementById("region-kind") as HTMLSelectElement;
  const color = document.getElementById("region-color") as HTMLInputElement;
  const save = screen.getByRole("button", { name: "Save" });
  const overlay = view.container.querySelector(".overlay") as HTMLElement;
  return { ...view, onSave, onCancel, name, kind, color, save, overlay };
}

describe("RegionDialog", () => {
  it("keeps saving disabled until the name is filled", () => {
    const { name, save } = setup();
    expect(save).toBeDisabled();

    fireEvent.change(name, { target: { value: "   " } });
    expect(save).toBeDisabled();

    fireEvent.change(name, { target: { value: "Intro" } });
    expect(save).toBeEnabled();
  });

  it("saves a trimmed name, the selected kind and its color", () => {
    const { name, kind, save, onSave } = setup();

    fireEvent.change(name, { target: { value: "  Verse  " } });
    fireEvent.change(kind, { target: { value: "chorus" } });
    fireEvent.click(save);

    expect(onSave).toHaveBeenCalledWith({
      name: "Verse",
      kind: "chorus",
      color: REGION_COLORS.chorus,
    });
  });

  it("saves with Enter and ignores Enter when the name is blank", () => {
    const { name, onSave } = setup();

    fireEvent.keyDown(name, { key: "Enter" });
    expect(onSave).not.toHaveBeenCalled();

    fireEvent.change(name, { target: { value: "Solo" } });
    fireEvent.keyDown(name, { key: "Enter" });
    expect(onSave).toHaveBeenCalledWith({
      name: "Solo",
      kind: "custom",
      color: REGION_COLORS.custom,
    });
  });

  it("cancels from the close button, the cancel button and the overlay", () => {
    const { onCancel, overlay } = setup();

    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    fireEvent.click(overlay);
    expect(onCancel).toHaveBeenCalledTimes(3);
  });

  it("does not close when clicking inside the dialog", () => {
    const { onCancel } = setup();
    fireEvent.click(screen.getByRole("dialog"));
    expect(onCancel).not.toHaveBeenCalled();
  });

  it("prefills from the initial draft and resets when it changes", () => {
    const { name, kind, color, rerender } = setup({
      name: "Chorus",
      kind: "chorus",
      color: "#ff8a4f",
    });
    expect(name.value).toBe("Chorus");
    expect(kind.value).toBe("chorus");
    expect(color.value).toBe("#ff8a4f");

    rerender(
      <RegionDialog
        initial={{ name: "Outro", kind: "outro", color: "#5fb98a" }}
        onCancel={vi.fn()}
        onSave={vi.fn()}
      />,
    );
    expect(name.value).toBe("Outro");
    expect(kind.value).toBe("outro");
    expect(color.value).toBe("#5fb98a");
  });

  it("applies a custom color to the draft", () => {
    const { name, color, save, onSave } = setup();
    fireEvent.change(name, { target: { value: "Custom" } });
    fireEvent.change(color, { target: { value: "#123456" } });
    fireEvent.click(save);
    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({ color: "#123456" }),
    );
  });
});
