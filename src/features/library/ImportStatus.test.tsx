import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ImportStatus from "./ImportStatus";

const state = vi.hoisted(() => ({
  value: {} as Record<string, unknown>,
}));

vi.mock("./ImportProvider", () => ({
  useImport: () => state.value,
}));

describe("ImportStatus", () => {
  beforeEach(() => {
    state.value = {};
  });

  it("renders nothing while no import is running", () => {
    const { container } = render(<ImportStatus />);
    expect(container).toBeEmptyDOMElement();
  });

  it("shows progress and cancels the job", async () => {
    const cancelImport = vi.fn();
    state.value = {
      importing: true,
      cancelling: false,
      progress: {
        jobId: 1,
        index: 2,
        total: 5,
        name: "song.mp3",
        copiedBytes: 10,
        totalBytes: 20,
        phase: "copying",
      },
      cancelImport,
    };
    render(<ImportStatus />);

    expect(
      screen.getByText("Добавление 2 из 5: song.mp3"),
    ).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Отменить" }));
    expect(cancelImport).toHaveBeenCalled();
  });

  it("disables the button while cancelling", () => {
    state.value = {
      importing: true,
      cancelling: true,
      progress: null,
      cancelImport: vi.fn(),
    };
    render(<ImportStatus />);

    expect(screen.getByText("Добавляем файлы…")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Отмена…" })).toBeDisabled();
  });
});
