import { expect, test } from "@playwright/test";

test("model editor keeps raw and structured views synchronized", async ({ page }) => {
  await page.goto("/#model/editor");
  await expect(page.getByRole("heading", { name: "Topology" })).toHaveCount(0);
  await expect(
    page.getByRole("complementary", { name: "Model documents" }),
  ).toBeVisible();
  const review = page.getByRole("complementary", { name: "Draft review" });
  if ((page.viewportSize()?.width ?? 0) <= 1080) {
    await expect(review).toBeHidden();
    await page.getByRole("button", { name: "Toggle draft review" }).click();
    await expect(review).toBeVisible();
    await page.getByRole("button", { name: "Close draft review" }).click();
    await expect(review).toBeHidden();
  } else {
    await expect(review).toBeVisible();
  }

  await page.getByRole("tab", { name: "YAML" }).click();
  const source = page.getByLabel("Raw YAML model document");
  await expect(source).toBeVisible();
  const original = await source.inputValue();
  await source.fill(original.replace(/id:\s*([^\n]+)/, "id: $1"));
  await page.getByRole("tab", { name: "Structured" }).click();
  await expect(page.getByLabel("Structured model document editor")).toBeVisible();
});

test("regional architecture exposes the model editor workflow", async ({ page }) => {
  await page.goto("/");
  await expect(
    page.getByRole("region", { name: "Hearthline regional map" }),
  ).toBeVisible();
  const markerSelector = (page.viewportSize()?.width ?? 0) <= 620
    ? ".mobile-place-marker"
    : ".place-marker";
  await page.locator(markerSelector).filter({ hasText: "Customer Network" }).click();
  await expect(
    page.getByRole("complementary", { name: "Selected location" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Close location details" }).click();
  const editor = page.getByRole("button", { name: /model editor/i });
  await expect(editor).toBeVisible();
  await editor.click();
  await expect(page).toHaveURL(/#model\/editor$/);
});
