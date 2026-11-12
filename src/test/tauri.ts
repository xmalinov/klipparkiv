import { act } from "@testing-library/react";
import type { Mock } from "vitest";

export type CommandArgs = Record<string, unknown> | undefined;
export type CommandHandler = (args: CommandArgs) => unknown;

/**
 * Routes `invoke` calls to per-command handlers and records every call, so a
 * test can assert both the behaviour and the exact payload sent to the backend.
 * Unknown commands resolve with `undefined` (the app treats most commands as
 * fire-and-forget), which keeps fixtures focused on the scenario under test.
 */
export function createCommandRouter(invoke: Mock) {
  const handlers = new Map<string, CommandHandler>();
  const history: { command: string; args: CommandArgs }[] = [];

  invoke.mockImplementation(async (command: string, args?: CommandArgs) => {
    history.push({ command, args });
    const handler = handlers.get(command);
    return handler ? await handler(args) : undefined;
  });

  return {
    /** Registers (or replaces) the handler for one backend command. */
    on(command: string, handler: CommandHandler) {
      handlers.set(command, handler);
      return this;
    },
    /** Makes the next call reject with the given message. */
    fail(command: string, message: string) {
      handlers.set(command, () => {
        throw new Error(message);
      });
      return this;
    },
    calls(command?: string) {
      return command
        ? history.filter((entry) => entry.command === command)
        : [...history];
    },
    args(command: string) {
      return this.calls(command).map((entry) => entry.args);
    },
    count(command: string) {
      return this.calls(command).length;
    },
    reset() {
      history.length = 0;
      return this;
    },
  };
}

export type RoutedCommands = ReturnType<typeof createCommandRouter>;

type EventHandler = (event: { payload: unknown }) => void;

/**
 * Stubs `listen` with an in-memory bus: tests can emit backend events and
 * check that listeners are removed on unmount.
 */
export function createEventBus(listen: Mock) {
  const handlers = new Map<string, Set<EventHandler>>();

  listen.mockImplementation(
    (event: string, handler: EventHandler) => {
      let bucket = handlers.get(event);
      if (!bucket) {
        bucket = new Set();
        handlers.set(event, bucket);
      }
      bucket.add(handler);
      return Promise.resolve(() => {
        bucket.delete(handler);
      });
    },
  );

  return {
    /** Delivers an event inside `act` and flushes the resulting updates. */
    async emit(event: string, payload: unknown) {
      await act(async () => {
        for (const handler of [...(handlers.get(event) ?? [])]) {
          handler({ payload });
        }
        await Promise.resolve();
      });
    },
    listenerCount(event?: string) {
      if (event) return handlers.get(event)?.size ?? 0;
      let total = 0;
      for (const bucket of handlers.values()) total += bucket.size;
      return total;
    },
  };
}

export type PracticeEventBus = ReturnType<typeof createEventBus>;

/** Flushes pending promises and React updates outside of an act warning. */
export async function flush() {
  await act(async () => {
    await Promise.resolve();
  });
}
