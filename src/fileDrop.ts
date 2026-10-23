type FileDropPayload =
  | { type: "enter" | "over" }
  | { type: "leave" }
  | { type: "drop"; paths: string[] };
type Unlisten = () => void;

// Registration is asynchronous. StrictMode can dispose an effect before it resolves.
export function listenForFileDrops(
  listen: (
    handler: (event: { payload: FileDropPayload }) => void,
  ) => Promise<Unlisten>,
  onHover: (hover: boolean) => void,
  onDrop: (paths: string[]) => void,
  onError: (error: unknown) => void,
): Unlisten {
  let disposed = false;
  let unlisten: Unlisten | undefined;
  void listen(({ payload }) => {
    if (disposed) return;
    onHover(payload.type === "enter" || payload.type === "over");
    if (payload.type === "drop" && payload.paths.length > 0)
      onDrop(payload.paths);
  })
    .then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    })
    .catch((error: unknown) => {
      if (!disposed) onError(error);
    });
  return () => {
    if (disposed) return;
    disposed = true;
    unlisten?.();
  };
}

export async function importDllPaths(
  paths: string[],
  importFile: (path: string) => Promise<unknown>,
): Promise<{
  count: number;
  updated: number;
  unchanged: number;
  errors: string[];
}> {
  let count = 0;
  let updated = 0;
  let unchanged = 0;
  const errors: string[] = [];
  for (const path of paths) {
    try {
      const outcome = await importFile(path);
      if (outcome === "updated") updated++;
      else if (outcome === "unchanged") unchanged++;
      else count++;
    } catch (error) {
      errors.push(`${path.split(/[\\/]/).pop()}: ${String(error)}`);
    }
  }
  return { count, updated, unchanged, errors };
}
