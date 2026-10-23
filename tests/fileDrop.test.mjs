import test from "node:test";
import assert from "node:assert/strict";
import { listenForFileDrops, importDllPaths } from "../src/fileDrop.ts";

test("native enter/over/leave and multi-file drop update hover and forward paths", async () => {
  let send;
  let stops = 0;
  const hover = [];
  const drops = [];
  const dispose = listenForFileDrops(
    async (handler) => {
      send = handler;
      return () => stops++;
    },
    (value) => hover.push(value),
    (paths) => drops.push(paths),
    assert.fail,
  );
  send({ payload: { type: "enter", paths: ["C:\\a.dll"] } });
  send({ payload: { type: "over" } });
  send({ payload: { type: "leave" } });
  send({
    payload: { type: "drop", paths: ["C:\\a.dll", "C:\\日本語\\b.DLL"] },
  });
  send({ payload: { type: "drop", paths: [] } });
  await Promise.resolve();
  dispose();
  dispose();
  send({ payload: { type: "drop", paths: ["ignored.dll"] } });
  assert.deepEqual(hover, [true, true, false, false, false]);
  assert.deepEqual(drops, [["C:\\a.dll", "C:\\日本語\\b.DLL"]]);
  assert.equal(stops, 1);
});

test("late asynchronous registration is cleaned up after StrictMode disposal", async () => {
  let complete;
  let send;
  let stops = 0;
  const dispose = listenForFileDrops(
    (handler) => {
      send = handler;
      return new Promise((resolve) => {
        complete = resolve;
      });
    },
    assert.fail,
    assert.fail,
    assert.fail,
  );
  dispose();
  send({ payload: { type: "drop", paths: ["ignored.dll"] } });
  complete(() => stops++);
  await Promise.resolve();
  assert.equal(stops, 1);
});

test("subscription errors are visible only while mounted", async () => {
  const errors = [];
  listenForFileDrops(
    async () => {
      throw Error("not permitted");
    },
    assert.fail,
    assert.fail,
    (e) => errors.push(e.message),
  );
  const dispose = listenForFileDrops(
    async () => {
      throw Error("disposed");
    },
    assert.fail,
    assert.fail,
    assert.fail,
  );
  dispose();
  await Promise.resolve();
  await Promise.resolve();
  assert.deepEqual(errors, ["not permitted"]);
});

test("mixed files import sequentially and continue after invalid or duplicate DLLs", async () => {
  const calls = [];
  let active = 0;
  const result = await importDllPaths(
    [
      "C:\\mods\\valid.dll",
      "C:\\mods\\text.txt",
      "C:\\mods\\duplicate.dll",
      "C:\\mods\\日本語.DLL",
    ],
    async (path) => {
      assert.equal(active++, 0);
      await Promise.resolve();
      calls.push(path);
      active--;
      if (path.endsWith("text.txt")) throw Error(".dll required");
      if (path.endsWith("duplicate.dll")) throw Error("already imported");
    },
  );
  assert.equal(calls.length, 4);
  assert.equal(result.count, 2);
  assert.deepEqual(result.errors, [
    "text.txt: Error: .dll required",
    "duplicate.dll: Error: already imported",
  ]);
});

test("empty drops import no files", async () => {
  assert.deepEqual(await importDllPaths([], assert.fail), {
    count: 0,
    updated: 0,
    unchanged: 0,
    errors: [],
  });
});
test("new files, updates and identical files are counted separately", async () => {
  const statuses = {
    "new.dll": "added",
    "update.dll": "updated",
    "same.dll": "unchanged",
  };
  assert.deepEqual(
    await importDllPaths(Object.keys(statuses), async (path) => statuses[path]),
    {
      count: 1,
      updated: 1,
      unchanged: 1,
      errors: [],
    },
  );
});
