// KOS-147 reference screenshots — copy this file into a cortex checkout at
// host/e2e/kos147-memoria-screens.spec.ts, then from host/:
//   KOSMOS_HOST_E2E_CLEANUP_MANIFEST=/tmp/kos147-cleanup.json \
//   KOS147_SNAPSHOT=/path/to/memoria-gpui/fixtures/ark-snapshot.json \
//   KOS147_SHOTS_DIR=/path/to/memoria-gpui/reference/screens \
//   xvfb-run -a ./node_modules/.bin/playwright test -c playwright.config.ts kos147-memoria-screens
// Requires: cargo build of kepler-backend + ark-core-rpc (see
// e2e/fixtures/host-runtime.ts in cortex) and a visible display — headless
// host windows are created hidden (show:false) and never produce frames.
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { test, expect } from "@playwright/test";
import { _electron as electron, type ElectronApplication, type Page } from "playwright";
import electronBinary from "electron";
import { createSignedApps } from "./fixtures/signed-apps";
import {
  buildEngine,
  cargoTarget,
  closeHost,
  executableName,
  hostE2eEnvironment,
  recordCleanup,
  rpc,
  rpcError,
  startEngine,
  terminate,
  waitForPidGone,
} from "./fixtures/host-runtime";

const hostRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const workspaceRoot = path.resolve(hostRoot, "..", "..");
const hostMain = path.join(hostRoot, "dist-electron", "main.js");
const SNAPSHOT =
  process.env.KOS147_SNAPSHOT ??
  "/workspace/wt/memoria-gpui-kos-147/fixtures/ark-snapshot.json";
const SHOTS = process.env.KOS147_SHOTS_DIR ?? "/home/box/devin-runs/kos-147/artifacts/screens";

test("KOS-147 memoria reference screenshots", async () => {
  test.setTimeout(600_000);
  test.skip(!fs.existsSync(hostMain), `build Host first: ${hostMain}`);
  test.skip(!fs.existsSync(SNAPSHOT), `fixture snapshot missing: ${SNAPSHOT}`);
  fs.mkdirSync(SHOTS, { recursive: true });

  const root = fs.mkdtempSync(path.join(os.tmpdir(), "kosmos-host-e2e-memoria-shots-"));
  const dataDir = path.join(root, "engine");
  const userData = path.join(root, "host-user-data");
  // No KOSMOS_HEADLESS/KOSMOS_TEST_MODE: headless host windows are created
  // hidden (show:false) and never produce compositor frames, so screenshots
  // would time out. xvfb-run provides the display instead.
  const environment = hostE2eEnvironment({
    APPDATA: path.join(root, "appdata"),
    XDG_CONFIG_HOME: path.join(root, "xdg-config"),
    KOSMOS_DATA_DIR: dataDir,
  });
  if (process.platform === "linux") environment.ELECTRON_DISABLE_SANDBOX = "1";
  const cleanupManifest = process.env.KOSMOS_HOST_E2E_CLEANUP_MANIFEST;
  if (!cleanupManifest) throw new Error("KOSMOS_HOST_E2E_CLEANUP_MANIFEST is required");
  recordCleanup(cleanupManifest, root, new Set());

  let host: ElectronApplication | undefined;
  let engine: Awaited<ReturnType<typeof startEngine>>["child"] | undefined;
  const pids = new Set<number>();
  const failedSeeds: string[] = [];

  const shot = async (page: Page, name: string) => {
    const file = path.join(SHOTS, `${name}.png`);
    await page.screenshot({ path: file });
    console.log(`[kos147] shot ${file}`);
  };
  const setTheme = async (page: Page, theme: "light" | "dark") => {
    await page.evaluate((t) => localStorage.setItem("memoria-theme", t), theme);
    await page.reload();
    await expect.poll(() => page.evaluate(() => Boolean(window.api)), { timeout: 30_000 }).toBe(true);
  };
  const launchHost = () =>
    electron.launch({
      executablePath: electronBinary,
      args: [`--user-data-dir=${userData}`, hostMain, "--open-app", "com.kosmos.memoria"],
      env: environment,
      timeout: 30_000,
    });
  // Routes are delivered to the existing window via host navigation
  // (kepler.apps.open → kepler:extension:navigation → memoria command bus).
  const nav = async (page: Page, route: string) => {
    await page.evaluate(
      (r) =>
        (window as unknown as { kepler: { apps: { open: (q: unknown) => Promise<unknown> } } })
          .kepler.apps.open({ id: "com.kosmos.memoria", route: r }),
      route,
    );
    await page.waitForTimeout(1500);
  };
  const openNote = (page: Page, id: string) => nav(page, `/note/${encodeURIComponent(id)}`);
  // Titlebar sits in a pointer-events:none chrome region; dispatch clicks
  // directly on the DOM node instead of Playwright hit-testing.
  const clickTestId = (page: Page, testid: string) =>
    page.evaluate(
      (t) => (document.querySelector(`[data-testid="${t}"]`) as HTMLElement | null)?.click(),
      testid,
    );

  try {
    const apps = createSignedApps(root, workspaceRoot, false, false, false, false, true);
    const binaries = buildEngine(apps.trust);
    const started = await startEngine(binaries.engine, binaries.ark, dataDir);
    engine = started.child;
    if (engine.pid) pids.add(engine.pid);
    const lock = started.lock;

    const catalog = await rpc(lock, "packages.catalog_apply", {
      document: apps.catalog,
      signatures: apps.signatures,
    });
    expect(catalog.ok, rpcError(catalog)).toBe(true);
    const install = await rpc(lock, "packages.install", {
      id: "com.kosmos.memoria",
      version: apps.versions["com.kosmos.memoria"],
      archive_path: apps.archives["com.kosmos.memoria"],
    });
    expect(install.ok, rpcError(install)).toBe(true);
    const enable = await rpc(lock, "packages.set_enabled", {
      id: "com.kosmos.memoria",
      version: apps.versions["com.kosmos.memoria"],
      enabled: true,
    });
    expect(enable.ok, rpcError(enable)).toBe(true);

    // --- pass 1: empty state (clean engine) --------------------------------
    host = await launchHost();
    pids.add(host.process().pid);
    let page = await host.firstWindow();
    await expect.poll(() => page.evaluate(() => Boolean(window.api)), { timeout: 30_000 }).toBe(true);
    await page.getByTestId("everything-add-card").waitFor({ state: "visible", timeout: 30_000 });
    await page.waitForTimeout(500);
    await shot(page, "everything-empty-dark");
    await setTheme(page, "light");
    await shot(page, "everything-empty-light");
    // Close the host BEFORE seeding: writes while the app is live show up as
    // remote mutations and trigger the "Изменено удалённо" conflict banner.
    await closeHost(host, pids);
    host = undefined;

    // --- seed fixtures ------------------------------------------------------
    const snapshot = JSON.parse(fs.readFileSync(SNAPSHOT, "utf8")) as {
      objects: Record<string, unknown>[];
      links: Record<string, unknown>[];
    };
    for (const object of snapshot.objects) {
      if (object.$seed === false) continue;
      const r = await rpc(lock, "upsert_object", { object });
      if (r.ok !== true) failedSeeds.push(`object ${object.id}: ${rpcError(r)}`);
    }
    for (const link of snapshot.links) {
      const r = await rpc(lock, "upsert_object_link", { object_link: link });
      if (r.ok !== true) failedSeeds.push(`link ${link.id}: ${rpcError(r)}`);
    }
    console.log(`[kos147] seeded ${snapshot.objects.length} objects, failures: ${failedSeeds.length}`);
    for (const f of failedSeeds) console.log(`[kos147] seed-fail ${f}`);

    // --- pass 2: populated screens ------------------------------------------
    host = await launchHost();
    pids.add(host.process().pid);
    page = await host.firstWindow();
    await expect.poll(() => page.evaluate(() => Boolean(window.api)), { timeout: 30_000 }).toBe(true);
    await page.getByTestId("everything-add-card").waitFor({ state: "visible", timeout: 30_000 });
    await page.waitForTimeout(1500);
    await shot(page, "everything-light");
    await setTheme(page, "dark");
    await page.getByTestId("everything-add-card").waitFor({ state: "visible", timeout: 30_000 });
    await shot(page, "everything-dark");

    // diary first — the top-nav pills are hidden while a note is open, so
    // all pill navigation must happen before we enter the editor.
    const openDiary = async () => {
      for (let i = 0; i < 4; i++) {
        await clickTestId(page, "top-nav-diary");
        try {
          await page.getByTestId("diary-view").waitFor({ state: "visible", timeout: 8_000 });
          return;
        } catch { /* retry */ }
      }
      await page.getByTestId("diary-view").waitFor({ state: "visible", timeout: 20_000 });
    };
    await openDiary();
    await page.waitForTimeout(800);
    await shot(page, "diary-dark");
    await setTheme(page, "light");
    await openDiary();
    await page.waitForTimeout(800);
    await shot(page, "diary-light");

    // editor — kitchen sink (routes work from any screen)
    await openNote(page, "note-markup-kitchen-sink");
    await shot(page, "editor-light");
    await setTheme(page, "dark");
    await page.waitForTimeout(800);
    await openNote(page, "note-markup-kitchen-sink");
    await shot(page, "editor-dark");

    // editor — empty note
    await openNote(page, "note-empty-title");
    await shot(page, "editor-empty-dark");

    // collection view (TypeObjectsView)
    await openNote(page, "collection:book_obj");
    await page.getByTestId("type-objects-view").waitFor({ state: "visible", timeout: 20_000 }).catch(() => {});
    await shot(page, "types-books-dark");
    await setTheme(page, "light");
    await page.waitForTimeout(500);
    await openNote(page, "collection:book_obj");
    await page.getByTestId("type-objects-view").waitFor({ state: "visible", timeout: 20_000 }).catch(() => {});
    await shot(page, "types-books-light");

    // book card detail
    await openNote(page, "book-1");
    await page.waitForTimeout(800);
    await shot(page, "book-light");

    // settings
    await page.evaluate(() => { window.location.hash = "#/settings"; });
    await page.getByTestId("eden-export-markdown").waitFor({ state: "visible", timeout: 20_000 }).catch(() => {});
    await page.waitForTimeout(800);
    await shot(page, "settings-light");
    await setTheme(page, "dark");
    await page.evaluate(() => { window.location.hash = "#/settings"; });
    await page.getByTestId("eden-export-markdown").waitFor({ state: "visible", timeout: 20_000 }).catch(() => {});
    await page.waitForTimeout(800);
    await shot(page, "settings-dark");

    // real aux sticker window via kepler.window.open (host sticker contract)
    const openOk = await page.evaluate(async () => {
      const api = (window as any).kepler?.window;
      if (!api?.open) return "no-api";
      return await api
        .open({
          key: "sticker:kos147",
          route: "/sticker/note-markup-kitchen-sink",
          width: 380,
          height: 480,
          minWidth: 260,
          minHeight: 200,
        })
        .catch((e: unknown) => String(e));
    });
    console.log("[kos147] sticker open:", JSON.stringify(openOk));
    if (openOk === true || (openOk as { ok?: boolean })?.ok === true) {
      // find the aux window (any page that isn't the main one)
      for (let i = 0; i < 20; i++) {
        const aux = host.windows().find((w) => w !== page);
        if (aux) {
          await aux.waitForLoadState("load").catch(() => {});
          await aux.waitForTimeout(2000);
          await shot(aux, "sticker-window-dark");
          break;
        }
        await page.waitForTimeout(500);
      }
    }

    // in-window sticker surface (hash route form)
    await page.evaluate(() => { window.location.hash = "#/sticker/note-markup-kitchen-sink"; });
    await page.getByTestId("sticker-note-window").waitFor({ state: "visible", timeout: 20_000 }).catch(() => {});
    await page.waitForTimeout(2000);
    await shot(page, "sticker-dark");
    await page.evaluate(() => { window.location.hash = "#/"; });

    // engine-down state
    await terminate(engine, path.join(cargoTarget(), "debug", executableName("kepler-backend")), dataDir, "Engine");
    engine = undefined;
    await page.evaluate(() => { window.location.hash = "#/"; });
    await page.reload().catch(() => {});
    await page.waitForTimeout(2500);
    await shot(page, "engine-down-dark");
    fs.writeFileSync(path.join(SHOTS, "seed-failures.json"), JSON.stringify(failedSeeds, null, 2));
  } finally {
    const cleanupErrors: unknown[] = [];
    const attempt = async (action: () => Promise<void>) => {
      try {
        await action();
      } catch (e) {
        cleanupErrors.push(e);
      }
    };
    if (host) pids.add(host.process().pid);
    await attempt(() => closeHost(host, pids));
    if (engine)
      await attempt(() =>
        terminate(engine, path.join(cargoTarget(), "debug", executableName("kepler-backend")), dataDir, "Engine"),
      );
    for (const pid of pids) await attempt(() => waitForPidGone(pid, "kos147 teardown"));
    try {
      recordCleanup(cleanupManifest, root, pids);
    } catch (e) {
      cleanupErrors.push(e);
    }
    for (const e of cleanupErrors) console.log(`[kos147] cleanup: ${e}`);
  }
});
