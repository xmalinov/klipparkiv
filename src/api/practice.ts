import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type RegionKind =
  | "intro"
  | "verse"
  | "chorus"
  | "solo"
  | "bridge"
  | "outro"
  | "custom";

export interface PracticeRegion {
  id: string;
  name: string;
  kind: RegionKind;
  color: string;
  start: number;
  end: number;
}

export interface PracticeState {
  version: number;
  positionSecs: number;
  speed: number;
  volume: number;
  pitchSemitones: number;
  cents: number;
  loopEnabled: boolean;
  preRoll: boolean;
  activeRegionId: string | null;
  regions: PracticeRegion[];
}

export interface PracticeOpened {
  path: string;
  durationSecs: number | null;
  sampleRate: number | null;
  state: PracticeState;
}

export interface PracticeSnapshot {
  positionSecs: number;
  durationSecs: number | null;
  playing: boolean;
  ended: boolean;
  underruns: number;
}

export interface WaveformLevel {
  stepSecs: number;
  peaks: string;
}

export interface Waveform {
  version: number;
  durationSecs: number;
  levels: WaveformLevel[];
}

export interface PracticeError {
  message: string;
}

export const DEFAULT_PRACTICE_STATE: PracticeState = {
  version: 1,
  positionSecs: 0,
  speed: 1,
  volume: 1,
  pitchSemitones: 0,
  cents: 0,
  loopEnabled: false,
  preRoll: true,
  activeRegionId: null,
  regions: [],
};

export function practiceOpen(path: string): Promise<PracticeOpened> {
  return invoke<PracticeOpened>("practice_open", { path });
}

export function practicePlay(): Promise<void> {
  return invoke<void>("practice_play");
}

export function practicePause(): Promise<void> {
  return invoke<void>("practice_pause");
}

export function practiceSeek(positionSecs: number): Promise<void> {
  return invoke<void>("practice_seek", { positionSecs });
}

export function practiceSetSpeed(speed: number): Promise<void> {
  return invoke<void>("practice_set_speed", { speed });
}

export function practiceSetPitch(semitones: number, cents: number): Promise<void> {
  return invoke<void>("practice_set_pitch", { semitones, cents });
}

export function practiceSetVolume(volume: number): Promise<void> {
  return invoke<void>("practice_set_volume", { volume });
}

export function practiceSetLoop(
  enabled: boolean,
  startSecs: number | null,
  endSecs: number | null,
  preRoll: boolean,
): Promise<void> {
  return invoke<void>("practice_set_loop", {
    enabled,
    startSecs,
    endSecs,
    preRoll,
  });
}

export function practicePosition(): Promise<PracticeSnapshot | null> {
  return invoke<PracticeSnapshot | null>("practice_position");
}

export function practiceClose(): Promise<void> {
  return invoke<void>("practice_close");
}

export function practiceStateGet(path: string): Promise<PracticeState> {
  return invoke<PracticeState>("practice_state_get", { path });
}

export function practiceStateSave(
  path: string,
  state: PracticeState,
): Promise<void> {
  return invoke<void>("practice_state_save", { path, state });
}

export function practiceRecent(limit = 20): Promise<
  {
    id: string;
    path: string;
    kind: "audio" | "video";
    title: string;
    channel: string | null;
    durationSecs: number | null;
    width: number | null;
    height: number | null;
    sizeBytes: number;
    mtime: number;
    thumbPath: string | null;
  }[]
> {
  return invoke("practice_recent", { limit });
}

export function practiceRecentHide(path: string): Promise<void> {
  return invoke<void>("practice_recent_hide", { path });
}

export function practiceWaveform(path: string): Promise<Waveform> {
  return invoke<Waveform>("practice_waveform", { path });
}

export function onPracticePosition(
  handler: (snapshot: PracticeSnapshot) => void,
): Promise<UnlistenFn> {
  return listen<PracticeSnapshot>("practice://position", (event) => {
    handler(event.payload);
  });
}

export function onPracticeError(
  handler: (error: PracticeError) => void,
): Promise<UnlistenFn> {
  return listen<PracticeError>("practice://error", (event) => {
    handler(event.payload);
  });
}

export function decodePeaks(encoded: string): Int8Array {
  const binary = atob(encoded);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return new Int8Array(bytes.buffer);
}
