import test from "node:test";
import assert from "node:assert/strict";
import {
  toggleModSelection,
  toggleVisibleSelection,
} from "../src/selection.ts";
test("click toggles a single item without changing hidden selection", () => {
  const original = new Set(["hidden"]);
  const selected = toggleModSelection(
    original,
    "a",
    ["a", "b"],
    undefined,
    false,
  );
  assert.deepEqual([...selected], ["hidden", "a"]);
  assert.deepEqual([...original], ["hidden"]);
  assert.deepEqual(
    [...toggleModSelection(selected, "a", ["a", "b"], "a", false)],
    ["hidden"],
  );
});
test("Shift click selects a forward or reverse visible range", () => {
  assert.deepEqual(
    [
      ...toggleModSelection(
        new Set(["a"]),
        "c",
        ["a", "b", "c", "d"],
        "a",
        true,
      ),
    ],
    ["a", "b", "c"],
  );
  assert.deepEqual(
    [
      ...toggleModSelection(
        new Set(["d"]),
        "b",
        ["a", "b", "c", "d"],
        "d",
        true,
      ),
    ].sort(),
    ["b", "c", "d"],
  );
});
test("Shift click on a selected item removes only that range", () => {
  assert.deepEqual(
    [
      ...toggleModSelection(
        new Set(["a", "b", "c", "hidden"]),
        "c",
        ["a", "b", "c"],
        "b",
        true,
      ),
    ],
    ["a", "hidden"],
  );
});
test("a filtered-out anchor falls back to a single selection", () => {
  assert.deepEqual(
    [...toggleModSelection(new Set(["hidden"]), "c", ["b", "c"], "a", true)],
    ["hidden", "c"],
  );
});
test("select all and clear affect visible items only", () => {
  const original = new Set(["hidden", "a"]);
  const selected = toggleVisibleSelection(original, ["a", "b"]);
  assert.deepEqual([...selected], ["hidden", "a", "b"]);
  assert.deepEqual(
    [...toggleVisibleSelection(selected, ["a", "b"])],
    ["hidden"],
  );
  assert.deepEqual([...toggleVisibleSelection(original, [])], [...original]);
});
