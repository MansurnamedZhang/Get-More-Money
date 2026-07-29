import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

async function render() {
  const workerUrl = new URL("../dist/server/index.js", import.meta.url);
  workerUrl.searchParams.set("test", `${process.pid}-${Date.now()}`);
  const { default: worker } = await import(workerUrl.href);

  return worker.fetch(
    new Request("http://localhost/", {
      headers: { accept: "text/html" },
    }),
    {
      ASSETS: {
        fetch: async () => new Response("Not found", { status: 404 }),
      },
    },
    {
      waitUntil() {},
      passThroughOnException() {},
    },
  );
}

test("server-renders the investment manager loading screen", async () => {
  const response = await render();
  assert.equal(response.status, 200);
  assert.match(response.headers.get("content-type") ?? "", /^text\/html\b/i);

  const html = await response.text();
  assert.match(html, /<title>SANYU INVEST · Personal Investment Management<\/title>/i);
  assert.match(html, /class="loading-screen"/);
  assert.match(html, /<img[^>]*src="\/sanyu-invest-mark\.png"[^>]*class="loading-logo"/);
  assert.match(html, /<img[^>]*alt="SANYU INVEST"/);
  assert.match(html, /正在连接本地投资账本/);
});

test("keeps the application loading screen and metadata in sync", async () => {
  const [css, page, layout] = await Promise.all([
    readFile(new URL("../app/globals.css", import.meta.url), "utf8"),
    readFile(new URL("../app/page.tsx", import.meta.url), "utf8"),
    readFile(new URL("../app/layout.tsx", import.meta.url), "utf8"),
  ]);

  assert.match(page, /function LoadingScreen/);
  assert.match(page, /正在连接本地投资账本/);
  assert.match(page, /sanyu-invest-appearance/);
  assert.match(page, /深蓝色/);
  assert.match(page, /黑金色/);
  assert.match(page, /function StandardTransactionForm/);
  assert.match(page, /\/transactions\/duplicate-check/);
  assert.match(page, /正负方向、现金金额、币种和复式分录均由系统生成/);
  assert.match(page, /function InstrumentTagEditor/);
  assert.match(page, /\/instrument-tags/);
  assert.match(page, /类别标签/);
  assert.match(page, /搜索标的或标签/);
  assert.match(page, /updateInstrumentFilters/);
  assert.match(page, /投资标的已清空/);
  assert.doesNotMatch(page, /已保留当前选中的标的/);
  assert.match(page, /settlementCashCandidatesFor/);
  assert.match(page, /digitalAsset \|\| item\.asset_type === "cash"/);
  assert.match(page, /item\.id !== instrument\.id/);
  assert.match(page, /查找目标配置/);
  assert.match(page, /查找决策日志/);
  assert.match(page, /\/targets\/\$\{target\.id\}/);
  assert.match(page, /\/decisions\/\$\{decision\.id\}/);
  assert.match(css, /\.loading-screen\s*\{/);
  assert.match(css, /\.loading-logo\s*\{/);
  assert.match(css, /\.generated-entry-preview\s*\{/);
  assert.match(css, /\.transaction-mode-banner\.standard\s*\{/);
  assert.match(css, /\.instrument-picker-filters\s*\{/);
  assert.match(css, /\.instrument-tag-badge\s*\{/);
  assert.match(css, /\.planning-search\s*\{/);
  assert.match(css, /\.planning-delete\s*\{/);
  assert.match(css, /html\[data-mode="dark"\]/);
  assert.match(css, /html\[data-theme="deep-green"\]/);
  assert.match(layout, /title:\s*"SANYU INVEST · Personal Investment Management"/);
  assert.match(layout, /url:\s*"\/sanyu-invest-mark\.png"/);
  assert.match(layout, /lang="zh-CN"/);
});
