// Groups by clicking (docs/phases/GROUPS_PLAN.md): a group made in Define > Groups, the selection put in,
// then the group's objects selected, hidden, shown and shown alone; the group in the model and its tables.

import { expect, test } from "@playwright/test";

import { bench, chooseTables, fill, menu, openApp } from "./helpers";

test("a group of frames: made, filled from the selection, selected, hidden and shown by name", async ({ page }) => {
  await openApp(page);
  await page.evaluate((m) => window.__osab!.open(m, "b26.json"), bench("b26_load_combinations", "model.json"));
  const frames = await page.evaluate(() => window.__osab!.state().model.frames.map((f) => f.id));
  const two = frames.slice(0, 2);
  await page.evaluate((ids) => window.__osab!.state().select(ids.map((id) => ({ kind: "frame" as const, id })), "replace"), two);

  await menu(page, "Define", "Groups");
  const dlg = page.getByRole("dialog");
  await expect(dlg.getByTestId("group-counts")).toContainText(`${frames.length} frames`); // All
  await dlg.getByRole("button", { name: "Add New…" }).click();
  await fill(page, "Name", "PAIR");
  await dlg.getByRole("button", { name: "Add", exact: true }).click();
  await expect(dlg.getByText("Group PAIR")).toBeVisible();
  await dlg.getByRole("button", { name: "Add Selected Objects" }).click();
  await expect(dlg.getByTestId("group-counts")).toHaveText("0 joints, 2 frames, 0 links, 0 areas");
  const state = () => page.evaluate(() => {
    const s = window.__osab!.state();
    return { selection: s.selection.frames, hidden: s.hidden.frames, groups: s.model.groups };
  });
  expect((await state()).groups).toEqual([{ id: "PAIR", color: expect.any(String), frames: two }]);

  // Select, hide, show, show only.
  await page.evaluate(() => window.__osab!.state().clearSelection());
  await dlg.getByRole("button", { name: "Select", exact: true }).click();
  expect((await state()).selection).toEqual(two);
  await dlg.getByRole("button", { name: "Hide", exact: true }).click();
  expect((await state()).hidden).toEqual(two);
  await dlg.getByRole("button", { name: "Show", exact: true }).click();
  expect((await state()).hidden).toEqual([]);
  await dlg.getByRole("button", { name: "Show Only" }).click();
  expect((await state()).hidden.sort()).toEqual(frames.filter((f) => !two.includes(f)).sort());
  await dlg.getByRole("button", { name: "Close", exact: true }).click();

  // Select > Select by Group opens the same window; the tables list the group.
  await menu(page, "Select", "Select by Group");
  await expect(page.getByRole("dialog", { name: "Groups" })).toBeVisible();
  await page.getByRole("dialog").getByRole("button", { name: "Close", exact: true }).click();
  await menu(page, "Display", "Tables");
  await chooseTables(page, ["Groups", "Group Assignments"]);
  await expect(page.getByTestId("table-groups").locator("tbody tr")).toHaveCount(1);
  await page.getByRole("dialog").getByRole("button", { name: "Group Assignments" }).click();
  await expect(page.getByTestId("table-groupAssignments").locator("tbody tr")).toHaveCount(2);

  // The run is not changed by the group.
  await page.getByRole("dialog").getByRole("button", { name: "Close", exact: true }).click();
  await page.keyboard.press("F5");
  await expect(page.getByRole("status")).toContainText("Analysis complete");
});
