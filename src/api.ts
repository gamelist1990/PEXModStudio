import { invoke, isTauri } from "@tauri-apps/api/core";
export interface Mod {
  id: string;
  name: string;
  filename: string;
  sha256: string;
  size: number;
  enabled: boolean;
}
export interface Snapshot {
  library: { gamePath: string; mods: Mod[] };
  installed: boolean;
  healthy: boolean;
  running: boolean;
  detected: boolean;
  gameVersion: string;
  dataPath: string;
  logs: string;
  loaderVersion: string;
}
export const desktop = isTauri();
export async function getSnapshot(): Promise<Snapshot> {
  if (!desktop)
    return {
      library: {
        gamePath: "C:\\XboxGames\\Minecraft for Windows\\Content",
        mods: [],
      },
      installed: false,
      healthy: false,
      running: false,
      detected: false,
      gameVersion: "デスクトップで検出",
      dataPath: "",
      logs: "",
      loaderVersion: "Ultimate ASI Loader 10.0.0 + PEX Bridge 0.1.0",
    };
  return invoke("snapshot");
}
export async function command(name: string, args?: Record<string, unknown>) {
  if (!desktop) throw new Error("この操作は Windows アプリで利用できます。");
  return invoke(name, args);
}
