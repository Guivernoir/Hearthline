import { expect, test } from "@playwright/test";

test("mould four shows its permanently connected slip feed", async ({ page }) => {
  await page.goto("/#hmis/area-02-hmi-04");
  const machine = page.getByRole("region", { name: "mould-04 live machine view" });
  await expect(machine).toBeVisible();
  const svg = machine.getByRole("img");
  await expect(svg.locator(".slip-pipe")).toHaveCount(2);
  await expect(svg.locator(".slip-inlet")).toBeVisible();
  await expect(machine.getByText("Slip fill valve", { exact: true })).toBeVisible();
  await expect(machine.getByText("Closed", { exact: true })).toBeVisible();
  await expect(machine.getByText("Fill probe", { exact: true })).toHaveCount(0);
});
