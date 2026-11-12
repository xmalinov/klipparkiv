import { render } from "@testing-library/react";
import { PracticeProvider, usePractice } from "../features/practice/PracticeProvider";

/** The value exposed by the real provider — inferred to stay in sync. */
export type PracticeApi = ReturnType<typeof usePractice>;

let current: PracticeApi | null = null;

function Probe() {
  current = usePractice();
  return null;
}

/**
 * Renders the real `PracticeProvider` with a probe consumer.
 * Backend calls must be mocked at the `invoke`/`listen` boundary — the
 * provider itself is never mocked here.
 */
export function renderPractice() {
  const view = render(
    <PracticeProvider>
      <Probe />
    </PracticeProvider>,
  );
  return {
    ...view,
    api(): PracticeApi {
      if (!current) throw new Error("PracticeProvider is not mounted");
      return current;
    },
  };
}

export function resetPracticeProbe() {
  current = null;
}
