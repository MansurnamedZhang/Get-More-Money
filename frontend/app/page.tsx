"use client";

import { FormEvent, ReactNode, useCallback, useEffect, useId, useRef, useState } from "react";
import Image from "next/image";

const CONFIGURED_API_BASE = process.env.NEXT_PUBLIC_API_BASE_URL ?? "http://localhost:3001/api/v1";

function apiBase() {
  if (typeof window === "undefined") return CONFIGURED_API_BASE;
  try {
    const url = new URL(CONFIGURED_API_BASE);
    const loopbackHosts = new Set(["localhost", "127.0.0.1", "::1"]);
    if (loopbackHosts.has(url.hostname) && loopbackHosts.has(window.location.hostname)) {
      url.hostname = window.location.hostname;
    }
    return url.toString().replace(/\/$/, "");
  } catch {
    return CONFIGURED_API_BASE.replace(/\/$/, "");
  }
}

type Section = "overview" | "portfolio" | "ledger" | "decisions" | "data" | "settings";
type DialogKind = "account" | "instrument" | "network" | "transaction" | "transaction-detail" | "price" | "fx" | "target" | "decision" | "review" | "source" | "job" | "api-manager";
type ApiManagerSeed = { mode?: "price" | "crypto" | "fx"; instrument_id?: string; base?: string; quote?: string };
type FxResponseMode = "single" | "currency_paths" | "currency_map" | "currency_list";
type PriceResponseMode = "asset_map" | "asset_list";
type User = { id: string; username: string; display_name: string };
type AuthStatus = { setup_required: boolean; authenticated: boolean; user: User | null };
type Account = { id: string; name: string; institution: string | null; account_type: string; base_currency: string; include_in_net_worth: boolean; is_active: boolean };
type Instrument = { id: string; symbol: string; name: string; asset_type: string; currency: string; exchange: string | null; network: string | null; contract_address?: string | null; precision?: number | null; is_active: boolean };
type InstrumentTag = { id: string; instrument_id: string; name: string; created_at: string };
type BlockchainNetwork = { id: string; code: string; name: string; is_active: boolean; created_at: string; updated_at: string };
type Leg = { account_id: string; instrument_id: string; leg_type: string; quantity: string; unit_price: string | null; price_currency: string | null; memo?: string | null };
type Transaction = { id: string; transaction_type: string; trade_at: string; source: string; external_id?: string | null; memo: string | null; status: string; reverses_transaction_id?: string | null; created_at: string; legs: Leg[] };
type Holding = { account_id: string; account_name: string; account_type: string; instrument_id: string; symbol: string; name: string; asset_type: string; currency: string; quantity: string; average_cost: string; price: string; price_source: string; price_at: string | null; market_value: string; cost_basis: string; unrealized_pnl: string; realized_pnl: string; weight: string; stale: boolean; missing_price: boolean; missing_fx: boolean };
type Allocation = { key: string; value: string; weight: string };
type Portfolio = { report_currency: string; total_market_value: string; investment_value: string; cash_value: string; total_cost_basis: string; holdings: Holding[]; allocation_by_asset_type: Allocation[]; allocation_by_account: Allocation[]; allocation_by_currency: Allocation[]; risk: { max_position_weight: string; crypto_weight: string; cash_weight: string; account_concentration: string; stale_price_count: number; missing_price_count: number; missing_fx_count: number; target_breaches: string[] }; performance: { xirr: string | null; twr: string | null; realized_pnl: string; unrealized_pnl: string; income: string; fees_and_taxes: string; note: string }; calculated_at: string };
type Price = { instrument_id: string; price_at: string; price: string; currency: string; source: string; is_manual_override: boolean };
type Fx = { base_currency: string; quote_currency: string; rate_at: string; rate: string; source: string };
type Settings = { report_currency: string; timezone: string; cost_method: string; stale_price_days: number; absolute_rebalance_threshold: string; relative_rebalance_threshold: string; transaction_hard_delete_minutes: number; updated_at: string };
type NetworkProxy = { is_enabled: boolean; protocol: "http" | "https" | "socks5"; host: string; port: number; updated_at: string };
type Appearance = { mode: "light" | "dark"; theme: "deep-blue" | "blue-black" | "deep-green" | "black-gold" };
type Policy = { objective: string; horizon_years: number; max_drawdown: string; max_single_position: string; max_high_risk: string; emergency_fund_months: number; allowed_tools: string; prohibited_tools: string; rebalance_frequency: string; review_frequency: string; updated_at: string };
type Target = { id: string; dimension: string; value: string; target_weight: string; min_weight: string; max_weight: string };
type Decision = { id: string; instrument_id: string | null; action: string; decided_at: string; rationale: string; confidence: number; risks: string; invalidation: string; review_at: string | null; outcome: string; process_score: number | null; result_score: number | null };
type Review = { id: string; period_type: string; period_start: string; period_end: string; summary: string; actions: string; completed_at: string | null };
type Source = { id: string; name: string; source_type: string; priority: number; credentials_ref: string | null; config: Record<string, unknown>; is_enabled: boolean };
type Job = { id: string; data_source_id: string; name: string; data_type: string; interval_seconds: number; timezone: string; next_run_at: string | null; last_run_at: string | null; is_enabled: boolean };
type SyncRun = { id: string; job_id: string; started_at: string; finished_at: string | null; status: string; stats_json: string; error_message: string | null };
type Collector = { id: string; source_id: string; name: string; source_type: string; priority: number; config: Record<string, unknown>; data_type: string; interval_seconds: number; timezone: string; next_run_at: string | null; last_run_at: string | null; is_enabled: boolean; created_at: string; updated_at: string; latest_run_status: string | null; latest_run_at: string | null; latest_error: string | null; has_api_key: boolean };
type CollectorTestResult = { success: boolean; provider: string; data_type: string; request_url: string; normalized_preview: Record<string, unknown> | Record<string, unknown>[]; record_count: number; used_api_key: boolean; elapsed_ms: number; tested_at: string };
type Snapshot = { accounts: Account[]; instruments: Instrument[]; instrumentTags: InstrumentTag[]; networks: BlockchainNetwork[]; transactions: Transaction[]; portfolio: Portfolio; prices: Price[]; fxRates: Fx[]; settings: Settings; networkProxy: NetworkProxy; policy: Policy; targets: Target[]; decisions: Decision[]; reviews: Review[]; sources: Source[]; jobs: Job[]; runs: SyncRun[]; collectors: Collector[] };
type AuditCounts = { accounts: number; active_accounts: number; instruments: number; active_instruments: number; transactions: number; transaction_legs: number; reversed_transactions: number; prices: number; fx_rates: number; import_batches: number; reconciliations: number; audit_logs: number; decisions: number; reviews: number; sync_runs: number };
type AuditCheck = { code: string; label: string; level: "critical" | "warning"; count: number; detail: string };
type AuditSummary = { generated_at: string; schema_version: string; period: { from: string | null; to: string | null }; report_currency: string; timezone: string; counts: AuditCounts; integrity: { passed: boolean; critical_issue_count: number; warning_count: number; checks: AuditCheck[] } };
type AuditPreset = "all" | "year" | "12m" | "custom";
type StandardTransactionType = "buy" | "sell" | "deposit" | "withdrawal" | "transfer" | "dividend" | "interest" | "return_of_capital" | "fee" | "tax" | "staking_reward" | "airdrop";
type TransactionPayload = { transaction_type: string; trade_at: string; source: string; external_id: string | null; memo: string | null; legs: { account_id: string; instrument_id: string; leg_type: string; quantity: string; unit_price: string | null; price_currency: string | null; memo: null }[] };
type DuplicateCheck = { duplicate: boolean; matches: { id: string; trade_at: string; memo: string | null; source: string }[] };

class ApiError extends Error { constructor(public status: number, message: string) { super(message); } }
async function api<T>(path: string, init: RequestInit = {}): Promise<T> {
  const headers = new Headers(init.headers);
  if (init.body && !headers.has("content-type")) headers.set("content-type", "application/json");
  const response = await fetch(`${apiBase()}${path}`, { ...init, headers, credentials: "include" });
  if (response.status === 204) return undefined as T;
  const body = await response.json().catch(() => null) as { error?: { message?: string } } | T | null;
  if (!response.ok) throw new ApiError(response.status, (body as { error?: { message?: string } } | null)?.error?.message ?? `请求失败（${response.status}）`);
  return body as T;
}

async function downloadApiFile(path: string, fallbackName: string) {
  const response = await fetch(`${apiBase()}${path}`, { credentials: "include" });
  if (!response.ok) {
    const body = await response.json().catch(() => null) as { error?: { message?: string } } | null;
    throw new ApiError(response.status, body?.error?.message ?? `导出失败（${response.status}）`);
  }
  const disposition = response.headers.get("content-disposition") ?? "";
  const matchedName = disposition.match(/filename="?([^";]+)"?/i)?.[1];
  const blob = await response.blob();
  const link = document.createElement("a");
  link.href = URL.createObjectURL(blob);
  link.download = matchedName ? decodeURIComponent(matchedName) : fallbackName;
  document.body.appendChild(link);
  link.click();
  link.remove();
  URL.revokeObjectURL(link.href);
}

const navigation: { id: Section; label: string; icon: string }[] = [
  { id: "overview", label: "总览", icon: "▦" }, { id: "portfolio", label: "组合", icon: "◉" }, { id: "ledger", label: "账本", icon: "⇄" },
  { id: "decisions", label: "计划与决策", icon: "◇" }, { id: "data", label: "数据", icon: "↻" }, { id: "settings", label: "设置", icon: "⚙" },
];
const sectionDescriptions: Record<Section, string> = {
  overview: "资产变化、数据质量与待办事项",
  portfolio: "持仓、收益与风险敞口",
  ledger: "不可变流水与交易记录",
  decisions: "投资纪律、目标与复盘",
  data: "行情、汇率与数据采集",
  settings: "系统偏好与安全设置",
};
const transactionLabels: Record<string, string> = { buy: "买入", sell: "卖出", deposit: "入金", withdrawal: "出金", transfer: "转账", dividend: "分红", interest: "利息", return_of_capital: "资本返还", fee: "费用", tax: "税费", staking_reward: "质押奖励", airdrop: "空投", corporate_action: "公司行动", adjustment: "调整", valuation: "估值" };
const assetLabels: Record<string, string> = { stock: "股票", etf: "ETF", fund: "基金", bond: "债券", cash: "现金", deposit: "存款", gold: "黄金", crypto: "虚拟货币", stablecoin: "稳定币", other: "其他" };
const accountLabels: Record<string, string> = { brokerage: "券商", bank: "银行", fund_platform: "基金平台", pension: "养老金", crypto_exchange: "交易所", self_custody_wallet: "自托管钱包", other: "其他" };
const providerLabels: Record<string, string> = { tencent_quote: "腾讯美股/港股行情", coingecko_simple: "CoinGecko", frankfurter: "Frankfurter", generic_json: "通用 JSON API" };
const majorCurrencies = [
  { code: "CNY", label: "人民币" },
  { code: "USD", label: "美元" },
  { code: "EUR", label: "欧元" },
  { code: "GBP", label: "英镑" },
  { code: "JPY", label: "日元" },
  { code: "HKD", label: "港币" },
  { code: "CHF", label: "瑞士法郎" },
  { code: "CAD", label: "加拿大元" },
  { code: "AUD", label: "澳大利亚元" },
  { code: "SGD", label: "新加坡元" },
  { code: "NZD", label: "新西兰元" },
] as const;
const commonCoinGeckoIds: Record<string, string> = {
  BTC: "bitcoin", ETH: "ethereum", USDT: "tether", USDC: "usd-coin", BNB: "binancecoin", SOL: "solana", XRP: "ripple", ADA: "cardano", DOGE: "dogecoin", AVAX: "avalanche-2", DOT: "polkadot", TRX: "tron", DAI: "dai", FDUSD: "first-digital-usd", PYUSD: "paypal-usd",
};
const investmentToolOptions = ["股票", "ETF", "基金", "债券", "现金", "定期存款", "黄金", "虚拟货币", "稳定币", "期权", "期货", "融资融券", "自动交易"];
const timezoneOptions = [{ value: "Asia/Shanghai", label: "中国标准时间" }, { value: "Asia/Hong_Kong", label: "香港时间" }, { value: "America/New_York", label: "纽约时间" }, { value: "Europe/London", label: "伦敦时间" }, { value: "UTC", label: "UTC" }];
const rebalanceFrequencyOptions = [{ value: "threshold", label: "达到偏离阈值时" }, { value: "monthly", label: "每月" }, { value: "quarterly", label: "每季度" }, { value: "semiannual", label: "每半年" }, { value: "annual", label: "每年" }];
const reviewFrequencyOptions = [{ value: "weekly", label: "每周" }, { value: "monthly", label: "每月" }, { value: "quarterly", label: "每季度" }, { value: "annual", label: "每年" }];
const appearanceThemes: { value: Appearance["theme"]; label: string; colors: [string, string] }[] = [{ value: "deep-blue", label: "深蓝色", colors: ["#092a4a", "#1677ff"] }, { value: "blue-black", label: "蓝黑色", colors: ["#050b14", "#2476d9"] }, { value: "deep-green", label: "深绿色", colors: ["#092c23", "#1d9b72"] }, { value: "black-gold", label: "黑金色", colors: ["#17130c", "#d2a13f"] }];

function number(value: string | number | null | undefined) { const parsed = Number(value ?? 0); return Number.isFinite(parsed) ? parsed : 0; }
function multiplyDecimalStrings(left: string, right: string) {
  const parse = (value: string) => {
    const match = value.trim().match(/^(\d+)(?:\.(\d+))?$/);
    if (!match) return null;
    const fraction = match[2] ?? "";
    return { integer: BigInt(`${match[1]}${fraction}`), scale: fraction.length };
  };
  const first = parse(left); const second = parse(right);
  if (!first || !second || first.integer === 0n || second.integer === 0n) return "";
  const scale = first.scale + second.scale;
  const digits = (first.integer * second.integer).toString().padStart(scale + 1, "0");
  if (scale === 0) return digits;
  const whole = digits.slice(0, -scale);
  const fraction = digits.slice(-scale).replace(/0+$/, "");
  return fraction ? `${whole}.${fraction}` : whole;
}
function money(value: string | number, currency = "CNY") { return new Intl.NumberFormat("zh-CN", { style: "currency", currency, maximumFractionDigits: 2 }).format(number(value)); }
function percent(value: string | number | null) { return value == null ? "数据不足" : `${(number(value) * 100).toFixed(2)}%`; }
function dateText(value: string | null | undefined) { return value ? new Intl.DateTimeFormat("zh-CN", { year: "numeric", month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" }).format(new Date(value)) : "—"; }
function localDateTime(value?: string | null) { const date = value ? new Date(value) : new Date(); return new Date(date.getTime() - date.getTimezoneOffset() * 60_000).toISOString().slice(0, 16); }
function errorMessage(error: unknown) { return error instanceof Error ? error.message : "操作失败"; }
function stockMarket(instrument: Instrument | undefined): "us" | "hk" | null { if (!instrument || !["stock", "etf"].includes(instrument.asset_type)) return null; if (instrument.currency === "HKD") return "hk"; if (instrument.currency === "USD") return "us"; return null; }
function stockQuoteSymbol(instrument: Instrument | undefined) { const market = stockMarket(instrument); if (!instrument || !market) return ""; let symbol = instrument.symbol.trim().toUpperCase(); if (market === "hk") { symbol = symbol.replace(/^HK/, "").replace(/\.HK$/, ""); return /^\d{1,5}$/.test(symbol) ? `hk${symbol.padStart(5, "0")}` : ""; } symbol = symbol.replace(/^US/, "").replace(/\.US$/, ""); return `us${symbol}`; }
function defaultCoinGeckoId(instrument: Instrument | undefined) { if (!instrument) return ""; return commonCoinGeckoIds[instrument.symbol.trim().toUpperCase()] ?? instrument.symbol.trim().toLowerCase(); }
function providerLabel(value: unknown) { const provider = String(value ?? ""); return providerLabels[provider] ?? provider; }
function normalizeMajorCurrency(value: string | null | undefined, fallback = "CNY") { const normalized = String(value ?? "").toUpperCase(); return majorCurrencies.some((currency) => currency.code === normalized) ? normalized : fallback; }
function currencyName(value: string | null | undefined) { const code = String(value ?? "").toUpperCase(); return majorCurrencies.find((currency) => currency.code === code)?.label ?? (code || "未设置"); }
function sourceLabel(value: string | null | undefined) { const source = String(value ?? "").trim(); const labels: Record<string, string> = { manual: "手工录入", tencent_quote: "腾讯行情", coingecko: "CoinGecko", coingecko_simple: "CoinGecko", frankfurter: "Frankfurter 汇率", ecb: "欧洲央行", demo: "测试数据", seed: "初始数据" }; return labels[source.toLowerCase()] ?? (providerLabel(source) || "未知来源"); }
function intervalLabel(seconds: number) { if (seconds < 3600) return `每 ${Math.max(1, Math.round(seconds / 60))} 分钟`; if (seconds < 86_400) return `每 ${Math.max(1, Math.round(seconds / 3600))} 小时`; const days = Math.max(1, Math.round(seconds / 86_400)); return days === 1 ? "每天" : `每 ${days} 天`; }
function compactNumber(value: string | number) { return new Intl.NumberFormat("zh-CN", { maximumFractionDigits: 8 }).format(number(value)); }
function runSummary(run: SyncRun) {
  if (run.error_message) return run.error_message;
  if (run.status === "running") return "正在连接并读取数据";
  try {
    const stats = JSON.parse(run.stats_json || "{}") as Record<string, unknown>;
    const count = Number(stats.records ?? stats.records_written ?? stats.prices_written ?? stats.pairs_written ?? 0);
    if (stats.test === true) return "连接测试已通过";
    if (Number.isFinite(count) && count > 0) return `已更新 ${count} 条数据`;
  } catch { /* 旧记录可能不是 JSON，统一使用友好描述。 */ }
  return run.status === "succeeded" ? "数据获取完成" : "本次获取未完成";
}
function splitSelections(value: string) { return value.split(/[,，、]/).map((item) => item.trim()).filter(Boolean); }
function splitNetworkCodes(value: string | null | undefined) { return String(value ?? "").split(/[,，、]/).map((item) => item.trim().toLowerCase().replace(/\s+/g, "-")).filter(Boolean); }

export default function Home() {
  const [auth, setAuth] = useState<AuthStatus | null>(null);
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [section, setSection] = useState<Section>("overview");
  const [dialog, setDialog] = useState<{ kind: DialogKind; value?: unknown } | null>(null);
  const [loading, setLoading] = useState(true);
  const [toast, setToast] = useState<string | null>(null);
  const [fatal, setFatal] = useState<string | null>(null);
  const [appearance, setAppearance] = useState<Appearance>({ mode: "light", theme: "deep-blue" });

  const load = useCallback(async () => {
    setLoading(true); setFatal(null);
    try {
      const status = await api<AuthStatus>("/auth/status"); setAuth(status);
      if (!status.authenticated) { setSnapshot(null); return; }
      const [accounts, instruments, transactions, portfolio, settings, networkProxy, policy] = await Promise.all([
        api<Account[]>("/accounts"), api<Instrument[]>("/instruments"), api<Transaction[]>("/transactions?limit=200&offset=0"), api<Portfolio>("/portfolio/summary"),
        api<Settings>("/settings"), api<NetworkProxy>("/network-proxy"), api<Policy>("/policy"),
      ]);
      const optionalLabels = ["标的标签", "区块链网络", "价格", "汇率", "配置目标", "决策", "复盘", "数据源", "同步任务", "运行记录", "API 采集器"];
      const optional = await Promise.allSettled([
        api<InstrumentTag[]>("/instrument-tags"), api<BlockchainNetwork[]>("/blockchain-networks"), api<Price[]>("/prices"), api<Fx[]>("/fx-rates"), api<Target[]>("/targets"), api<Decision[]>("/decisions"),
        api<Review[]>("/reviews"), api<Source[]>("/data-sources"), api<Job[]>("/sync-jobs"), api<SyncRun[]>("/sync-runs"), api<Collector[]>("/api-collectors"),
      ]);
      const value = <T,>(index: number, fallback: T) => optional[index].status === "fulfilled" ? optional[index].value as T : fallback;
      const failed = optional.flatMap((result, index) => result.status === "rejected" ? [optionalLabels[index]] : []);
      setSnapshot({
        accounts, instruments, transactions, portfolio, settings, networkProxy, policy,
        instrumentTags: value(0, []), networks: value(1, []), prices: value(2, []), fxRates: value(3, []), targets: value(4, []), decisions: value(5, []),
        reviews: value(6, []), sources: value(7, []), jobs: value(8, []), runs: value(9, []), collectors: value(10, []),
      });
      if (failed.length) setFatal(`部分辅助数据暂时不可用：${failed.join("、")}。核心账本仍可使用。`);
    } catch (error) {
      if (error instanceof ApiError && error.status === 401) setAuth((current) => ({ setup_required: current?.setup_required ?? false, authenticated: false, user: null }));
      else setFatal(errorMessage(error));
    } finally { setLoading(false); }
  }, []);

  useEffect(() => { const timer = window.setTimeout(() => { void load(); }, 0); return () => window.clearTimeout(timer); }, [load]);
  useEffect(() => { const timer = window.setTimeout(() => { try { const stored = JSON.parse(window.localStorage.getItem("sanyu-invest-appearance") ?? "null") as Partial<Appearance> | null; const next: Appearance = { mode: stored?.mode === "dark" ? "dark" : "light", theme: appearanceThemes.some((item) => item.value === stored?.theme) ? stored!.theme as Appearance["theme"] : "deep-blue" }; setAppearance(next); document.documentElement.dataset.mode = next.mode; document.documentElement.dataset.theme = next.theme; } catch { document.documentElement.dataset.mode = "light"; document.documentElement.dataset.theme = "deep-blue"; } }, 0); return () => window.clearTimeout(timer); }, []);
  useEffect(() => { if (!auth?.authenticated) return; const timer = window.setInterval(() => { void load(); }, 60_000); return () => window.clearInterval(timer); }, [auth?.authenticated, load]);
  useEffect(() => { if (!toast) return; const timer = window.setTimeout(() => setToast(null), 2600); return () => window.clearTimeout(timer); }, [toast]);

  if (!auth || (loading && !snapshot)) return <LoadingScreen message="正在连接本地投资账本…" />;
  if (!auth.authenticated) return <AuthScreen setup={auth.setup_required} onSuccess={() => void load()} />;
  if (!snapshot) return <LoadingScreen message={fatal ?? "正在读取组合数据…"} retry={fatal ? () => void load() : undefined} />;

  const saved = async (message: string) => { setDialog(null); setToast(message); await load(); };
  const open = (kind: DialogKind, value?: unknown) => setDialog({ kind, value });
  const changeAppearance = (next: Appearance) => { setAppearance(next); document.documentElement.dataset.mode = next.mode; document.documentElement.dataset.theme = next.theme; window.localStorage.setItem("sanyu-invest-appearance", JSON.stringify(next)); };
  const switchCurrency = async (currency: string) => {
    if (currency === snapshot.settings.report_currency) return;
    setLoading(true); setFatal(null);
    try {
      await api("/settings", { method: "PUT", body: JSON.stringify({ ...snapshot.settings, report_currency: currency }) });
      setToast(`核心币种已切换为 ${currency}`);
      await load();
    } catch (error) { setFatal(errorMessage(error)); setLoading(false); }
  };
  return (
    <div className="app-shell" data-od-id="investment-app-shell">
      <aside className="sidebar" data-od-id="primary-sidebar">
        <div className="brand" data-od-id="product-brand"><Image className="brand-logo" src="/sanyu-invest-mark.png" alt="SANYU INVEST" width={58} height={58} priority unoptimized /><div><strong>SANYU INVEST</strong><small>Personal Investment</small></div></div>
        <nav aria-label="主导航" data-od-id="primary-navigation">{navigation.map((item) => <button key={item.id} data-od-id={`nav-${item.id}`} title={item.label} aria-current={section === item.id ? "page" : undefined} className={section === item.id ? "active" : ""} onClick={() => setSection(item.id)}><i>{item.icon}</i>{item.label}</button>)}</nav>
        <div className="sidebar-bottom"><div className="connection"><i /><span><strong>本地账本已连接</strong><small>{snapshot.accounts.length} 个账户 · {snapshot.transactions.length} 条流水</small></span></div><div className="user-card"><b>{auth.user?.display_name.slice(0, 1)}</b><span><strong>{auth.user?.display_name}</strong><small>@{auth.user?.username}</small></span></div></div>
      </aside>
      <main data-od-id="primary-content">
        <header className="topbar" data-od-id="page-toolbar">
          <div className="page-context">
            <p>{new Intl.DateTimeFormat("zh-CN", { dateStyle: "full" }).format(new Date())}</p>
            <h1 data-od-id="page-title">{navigation.find((item) => item.id === section)?.label}</h1>
            <span>{sectionDescriptions[section]}</span>
          </div>
          <div className="topbar-actions">
            <label className="currency-switch"><span>核心币种</span><select value={snapshot.settings.report_currency} onChange={(event) => void switchCurrency(event.target.value)} disabled={loading}>{majorCurrencies.map((currency) => <option key={currency.code} value={currency.code}>{currency.code} · {currency.label}</option>)}</select></label>
            <button className="icon-btn" data-od-id="refresh-data" onClick={() => void load()} title="刷新页面数据；页面每 60 秒自动更新">{loading ? "◌" : "↻"}</button>
            <button className="primary" data-od-id="add-transaction" onClick={() => open("transaction")}>＋ 新增流水</button>
          </div>
        </header>
        {fatal && <div className="error-banner"><span>{fatal}</span><button onClick={() => void load()}>重试</button></div>}
        {section === "overview" && <Overview data={snapshot} open={open} go={setSection} />}
        {section === "portfolio" && <PortfolioView data={snapshot} open={open} />}
        {section === "ledger" && <LedgerView data={snapshot} open={open} onChanged={saved} />}
        {section === "decisions" && <DecisionView data={snapshot} open={open} onChanged={saved} />}
        {section === "data" && <CurrencyDataView data={snapshot} open={open} onChanged={saved} />}
        {section === "settings" && <SettingsView data={snapshot} user={auth.user!} appearance={appearance} onAppearanceChange={changeAppearance} onSaved={saved} onLogout={async () => { await api("/auth/logout", { method: "POST" }); setSnapshot(null); await load(); }} />}
      </main>
      <nav className="mobile-nav" aria-label="移动端主导航" data-od-id="mobile-navigation">{navigation.map((item) => <button key={item.id} aria-current={section === item.id ? "page" : undefined} className={section === item.id ? "active" : ""} onClick={() => setSection(item.id)}><i>{item.icon}</i><small>{item.label}</small></button>)}</nav>
      {dialog && <EditorDialog dialog={dialog} data={snapshot} close={() => setDialog(null)} saved={saved} />}
      {toast && <div className="toast">✓ {toast}</div>}
    </div>
  );
}

function LoadingScreen({ message, retry }: { message: string; retry?: () => void }) { return <div className="loading-screen"><Image className="loading-logo" src="/sanyu-invest-mark.png" alt="SANYU INVEST" width={116} height={116} priority unoptimized /><h1>SANYU INVEST</h1><p>{message}</p>{retry && <button onClick={retry}>重新连接</button>}</div>; }

function AuthScreen({ setup, onSuccess }: { setup: boolean; onSuccess: () => void }) {
  const [username, setUsername] = useState(setup ? "hans" : ""); const [displayName, setDisplayName] = useState("Hans"); const [password, setPassword] = useState(""); const [confirm, setConfirm] = useState(""); const [error, setError] = useState(""); const [busy, setBusy] = useState(false);
  async function submit(event: FormEvent) { event.preventDefault(); if (setup && password !== confirm) { setError("两次输入的密码不一致"); return; } setBusy(true); setError(""); try { await api(setup ? "/auth/setup" : "/auth/login", { method: "POST", body: JSON.stringify(setup ? { username, display_name: displayName, password } : { username, password }) }); onSuccess(); } catch (requestError) { setError(errorMessage(requestError)); } finally { setBusy(false); } }
  return <div className="auth-page"><section className="auth-story"><div className="brand light"><Image className="brand-logo" src="/sanyu-invest-mark.png" alt="SANYU INVEST" width={82} height={82} priority unoptimized /><div><strong>SANYU INVEST</strong><small>Personal Investment Management</small></div></div><div><p>本地优先 · 账本为真源</p><h1>把资产、收益与投资纪律，放在同一个可信账本里。</h1><ul><li>数据默认只保存在本机</li><li>所有修改保留完整审计轨迹</li><li>不保存券商或交易所登录密码</li></ul></div><small>系统提供记录与决策支持，不构成投资建议。</small></section><section className="auth-panel"><form onSubmit={submit}><p>{setup ? "首次使用" : "欢迎回来"}</p><h2>{setup ? "创建本地管理员" : "登录 SANYU INVEST"}</h2><span>{setup ? "此账户只用于保护本机投资数据。" : "请输入本地账户凭据。"}</span><label>用户名<input autoComplete="username" value={username} onChange={(event) => setUsername(event.target.value)} required /></label>{setup && <label>显示名称<input value={displayName} onChange={(event) => setDisplayName(event.target.value)} required /></label>}<label>密码<input type="password" autoComplete={setup ? "new-password" : "current-password"} value={password} onChange={(event) => setPassword(event.target.value)} required minLength={10} /></label>{setup && <label>确认密码<input type="password" autoComplete="new-password" value={confirm} onChange={(event) => setConfirm(event.target.value)} required /></label>}{error && <div className="form-error">{error}</div>}<button className="primary wide" disabled={busy}>{busy ? "正在处理…" : setup ? "创建并进入系统" : "登录"}</button>{setup && <small>密码至少 10 位，并同时包含字母和数字。</small>}</form></section></div>;
}

function Overview({ data, open, go }: { data: Snapshot; open: (kind: DialogKind, value?: unknown) => void; go: (section: Section) => void }) {
  const p = data.portfolio;
  const alerts = p.risk.missing_price_count + p.risk.missing_fx_count + p.risk.stale_price_count + p.risk.target_breaches.length;
  const pricedHoldings = p.holdings.filter((holding) => !holding.missing_price).length;
  const coverage = p.holdings.length ? pricedHoldings / p.holdings.length : 0;
  const costReturn = number(p.total_cost_basis) > 0 ? number(p.performance.unrealized_pnl) / number(p.total_cost_basis) : null;
  return <div className="dashboard-grid overview-grid">
    <section className="card overview-hero span-8" data-od-id="overview-net-worth">
      <div className="overview-hero-copy"><div className="section-head"><div><p>组合净值 · {p.report_currency}</p><h2>{money(p.total_market_value, p.report_currency)}</h2></div><span className="overview-status"><i />账本实时计算</span></div><div className={`return-chip ${number(p.performance.unrealized_pnl) >= 0 ? "up" : "down"}`}><strong>{money(p.performance.unrealized_pnl, p.report_currency)}</strong><span>未实现盈亏 · {percent(costReturn)}</span></div><div className="overview-kpis"><div><small>投资资产</small><strong>{money(p.investment_value, p.report_currency)}</strong></div><div><small>现金与稳定币</small><strong>{money(p.cash_value, p.report_currency)}</strong></div><div><small>总成本</small><strong>{money(p.total_cost_basis, p.report_currency)}</strong></div></div><div className="coverage-line"><span><strong>估值覆盖</strong><small>{pricedHoldings}/{p.holdings.length || 0} 个持仓有价格</small></span><div><i style={{ width: `${coverage * 100}%` }} /></div><b>{percent(coverage)}</b></div></div>
      <AllocationDonut items={p.allocation_by_asset_type} total={p.total_market_value} currency={p.report_currency} />
    </section>
    <section className="card span-4 attention overview-attention" data-od-id="overview-action-center"><div className="section-head"><div><p>行动中心</p><h3>{alerts ? `${alerts} 项需处理` : "组合状态良好"}</h3></div><b>{alerts}</b></div><AlertRows data={data} />{p.risk.target_breaches.slice(0, 2).map((item) => <div className="overview-breach" key={item}>策略越界 · {item}</div>)}<button className="soft" onClick={() => go("data")}>检查数据与规则 →</button></section>
    <section className="card span-12 quick-panel overview-quick-panel" data-od-id="overview-quick-actions"><div className="section-head"><div><p>快捷操作</p><h3>记录变化，保持数据可信</h3></div><span className="chart-caption">更新于 {dateText(p.calculated_at)}</span></div><div className="quick-actions"><button onClick={() => open("transaction")}><i>⇄</i><span><strong>新增流水</strong><small>买卖、收支、转账与费用</small></span></button><button onClick={() => open("price")}><i>¥</i><span><strong>录入价格</strong><small>更新持仓市值</small></span></button><button onClick={() => open("fx")}><i>↗</i><span><strong>录入汇率</strong><small>完成多币种换算</small></span></button><button onClick={() => open("review")}><i>◇</i><span><strong>记录复盘</strong><small>沉淀行动与经验</small></span></button></div></section>
    <section className="card span-7 position-map-card"><div className="section-head"><div><p>持仓权重地图</p><h3>集中度一眼可见</h3></div><button className="link-btn" onClick={() => go("portfolio")}>组合详情 →</button></div><PositionMap holdings={p.holdings} currency={p.report_currency} /></section>
    <section className="card span-5"><div className="section-head"><div><p>风险仪表</p><h3>四项核心敞口</h3></div><span className="chart-caption">当前组合</span></div><RiskCockpit risk={p.risk} maxSingle={number(data.policy.max_single_position)} maxHighRisk={number(data.policy.max_high_risk)} /></section>
    <section className="card span-8"><ExposureExplorer currency={p.report_currency} byCurrency={p.allocation_by_currency} byAccount={p.allocation_by_account} /></section>
    <section className="card span-4"><div className="section-head"><div><p>最近流水</p><h3>{data.transactions.length} 条有效记录</h3></div><button className="link-btn" onClick={() => go("ledger")}>完整账本 →</button></div><TransactionList transactions={data.transactions.slice(0, 5)} accounts={data.accounts} instruments={data.instruments} /></section>
  </div>;
}

function PortfolioView({ data, open }: { data: Snapshot; open: (kind: DialogKind, value?: unknown) => void }) { const p = data.portfolio; return <div className="page-stack"><div className="metric-grid"><Metric label="组合市值" value={money(p.total_market_value, p.report_currency)} detail={`计算于 ${dateText(p.calculated_at)}`} /><Metric label="总成本" value={money(p.total_cost_basis, p.report_currency)} detail="移动平均成本" /><Metric label="XIRR" value={percent(p.performance.xirr)} detail="资金加权年化" /><Metric label="现金比例" value={percent(p.risk.cash_weight)} detail="含稳定币" /></div><section className="card"><div className="section-head"><div><p>当前持仓</p><h3>{p.holdings.length} 个账户内持仓</h3></div><button className="primary small" onClick={() => open("price")}>更新价格</button></div><HoldingsTable holdings={p.holdings} currency={p.report_currency} full /></section><div className="two-column"><section className="card"><div className="section-head"><div><p>资产配置</p><h3>类别分布</h3></div></div><AllocationBars items={p.allocation_by_asset_type} /></section><section className="card"><div className="section-head"><div><p>风险检查</p><h3>可执行风险指标</h3></div></div><div className="risk-grid"><Risk label="最大单一仓位" value={percent(p.risk.max_position_weight)} /><Risk label="虚拟货币敞口" value={percent(p.risk.crypto_weight)} /><Risk label="账户集中度" value={percent(p.risk.account_concentration)} /><Risk label="价格异常" value={`${p.risk.missing_price_count + p.risk.stale_price_count} 项`} /></div>{p.risk.target_breaches.map((item) => <div className="warning-row" key={item}>! {item}</div>)}</section></div><section className="card"><div className="section-head"><div><p>收益口径</p><h3>盈亏与现金收益</h3></div></div><div className="metric-grid inner"><Metric label="已实现盈亏" value={money(p.performance.realized_pnl, p.report_currency)} detail="卖出结算" /><Metric label="未实现盈亏" value={money(p.performance.unrealized_pnl, p.report_currency)} detail="当前估值" /><Metric label="分红与利息" value={money(p.performance.income, p.report_currency)} detail="内部现金收益" /><Metric label="费用与税费" value={money(p.performance.fees_and_taxes, p.report_currency)} detail="累计支出" /></div><p className="note">{p.performance.note}</p></section></div>; }

function LedgerView({ data, open, onChanged }: { data: Snapshot; open: (kind: DialogKind, value?: unknown) => void; onChanged: (message: string) => Promise<void> }) {
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState("all");
  const [importing, setImporting] = useState(false);
  const [voidingId, setVoidingId] = useState<string | null>(null);
  const [hardDeletingId, setHardDeletingId] = useState<string | null>(null);
  const [correctionClock, setCorrectionClock] = useState(() => Date.now());
  useEffect(() => {
    if (!data.settings.transaction_hard_delete_minutes) return;
    const timer = window.setInterval(() => setCorrectionClock(Date.now()), 30_000);
    return () => window.clearInterval(timer);
  }, [data.settings.transaction_hard_delete_minutes]);
  const filtered = data.transactions.filter((tx) => {
    const text = `${transactionLabels[tx.transaction_type] ?? tx.transaction_type}${tx.memo ?? ""}${tx.legs.map((leg) => data.instruments.find((item) => item.id === leg.instrument_id)?.symbol ?? "").join("")}`.toLowerCase();
    return text.includes(query.toLowerCase()) && (filter === "all" || tx.transaction_type === filter);
  });
  async function importCsv(file: File) {
    setImporting(true);
    try {
      const csv = await file.text();
      const preview = await api<{ groups: number; valid_groups: number; duplicate_groups: number; errors: string[] }>("/imports/transactions?commit=false&source=csv", { method: "POST", headers: { "content-type": "text/csv; charset=utf-8" }, body: csv });
      if (preview.errors.length) throw new Error(preview.errors.join("\n"));
      if (!window.confirm(`识别 ${preview.groups} 笔流水，其中 ${preview.valid_groups} 笔可导入、${preview.duplicate_groups} 笔重复。确认入账？`)) return;
      const result = await api<{ imported_groups: number }>("/imports/transactions?commit=true&source=csv", { method: "POST", headers: { "content-type": "text/csv; charset=utf-8" }, body: csv });
      await onChanged(`已导入 ${result.imported_groups} 笔流水`);
    } catch (error) { alert(errorMessage(error)); } finally { setImporting(false); }
  }
  async function voidTransaction(tx: Transaction) {
    if (!window.confirm(`确认撤销这条“${transactionLabels[tx.transaction_type] ?? tx.transaction_type}”流水？\n\n系统会生成反向分录并保留审计记录，持仓和收益将立即重新计算。`)) return;
    setVoidingId(tx.id);
    try {
      await api<void>(`/transactions/${tx.id}`, { method: "DELETE" });
      await onChanged("流水已撤销，反向分录与审计记录已保留");
    } catch (error) { alert(errorMessage(error)); } finally { setVoidingId(null); }
  }
  async function permanentlyDeleteTransaction(tx: Transaction) {
    if (!window.confirm(`确认彻底删除这条“${transactionLabels[tx.transaction_type] ?? tx.transaction_type}”流水？\n\n流水及全部分录会立即移除，无法恢复；审计日志仅记录本次删除动作。`)) return;
    setHardDeletingId(tx.id);
    try {
      await api<void>(`/transactions/${tx.id}/permanent`, { method: "DELETE" });
      await onChanged("误录流水及其分录已彻底删除");
    } catch (error) { alert(errorMessage(error)); } finally { setHardDeletingId(null); }
  }
  return <div className="page-stack"><section className="card ledger"><div className="ledger-top"><div><p>不可变统一账本</p><h3>{data.transactions.length} 条有效流水</h3></div><div><input placeholder="搜索类型、标的或备注" value={query} onChange={(event) => setQuery(event.target.value)} /><button className="soft compact" onClick={() => downloadCsvTemplate(data)}>CSV 模板</button><label className="soft compact file-button">{importing ? "导入中…" : "导入 CSV"}<input type="file" accept=".csv,text/csv" disabled={importing} onChange={(event) => { const file = event.target.files?.[0]; if (file) void importCsv(file); event.target.value = ""; }} /></label><button className="primary" onClick={() => open("transaction")}>＋ 新增流水</button></div></div><div className="chips"><button className={filter === "all" ? "active" : ""} onClick={() => setFilter("all")}>全部</button>{["buy", "sell", "deposit", "withdrawal", "transfer", "dividend", "fee"].map((type) => <button key={type} className={filter === type ? "active" : ""} onClick={() => setFilter(type)}>{transactionLabels[type]}</button>)}</div><div className="ledger-rows">{filtered.map((tx) => <LedgerRow key={tx.id} tx={tx} data={data} now={correctionClock} hardDeleteMinutes={data.settings.transaction_hard_delete_minutes} view={() => open("transaction-detail", tx)} edit={() => open("transaction", tx)} voidEntry={() => void voidTransaction(tx)} hardRemove={() => void permanentlyDeleteTransaction(tx)} voiding={voidingId === tx.id} hardDeleting={hardDeletingId === tx.id} />)}{filtered.length === 0 && <Empty title="没有匹配的流水" action="新增第一条流水" onAction={() => open("transaction")} />}</div></section><div className="info-strip"><strong>账本操作说明</strong><span>编辑会冲销并重记；撤销会生成反向分录，两者都保留完整审计链。刚刚手工误录的流水可在 {data.settings.transaction_hard_delete_minutes ? `${data.settings.transaction_hard_delete_minutes} 分钟` : "关闭"}纠错窗口内彻底删除。</span></div></div>;
}

function DecisionView({ data, open, onChanged }: { data: Snapshot; open: (kind: DialogKind, value?: unknown) => void; onChanged: (message: string) => Promise<void> }) {
  const [targetQuery, setTargetQuery] = useState("");
  const [targetDimension, setTargetDimension] = useState("all");
  const [decisionQuery, setDecisionQuery] = useState("");
  const [deletingTargetId, setDeletingTargetId] = useState<string | null>(null);
  const [deletingDecisionId, setDeletingDecisionId] = useState<string | null>(null);
  const dimensionLabels: Record<string, string> = { asset_type: "资产类别", currency: "币种", account: "账户" };
  const targetDimensions = Array.from(new Set(data.targets.map((target) => target.dimension)));
  const targetValueLabel = (target: Target) => target.dimension === "asset_type" ? assetLabels[target.value] ?? target.value : target.dimension === "currency" ? currencyName(target.value) : target.dimension === "account" ? data.accounts.find((account) => account.id === target.value)?.name ?? target.value : target.value;
  const visibleTargets = data.targets.filter((target) => {
    const searchable = `${targetValueLabel(target)} ${target.value} ${dimensionLabels[target.dimension] ?? target.dimension}`.toLowerCase();
    return (!targetQuery.trim() || searchable.includes(targetQuery.trim().toLowerCase())) && (targetDimension === "all" || target.dimension === targetDimension);
  });
  const instrumentForDecision = (decision: Decision) => data.instruments.find((instrument) => instrument.id === decision.instrument_id);
  const visibleDecisions = data.decisions.filter((decision) => {
    const instrument = instrumentForDecision(decision);
    const searchable = `${decision.action} ${decision.rationale} ${decision.risks} ${decision.invalidation} ${decision.outcome} ${instrument?.symbol ?? ""} ${instrument?.name ?? ""}`.toLowerCase();
    return !decisionQuery.trim() || searchable.includes(decisionQuery.trim().toLowerCase());
  });

  const deleteTarget = async (target: Target) => {
    if (!window.confirm(`确认删除目标配置“${targetValueLabel(target)}”吗？\n\n删除后不再参与配置偏离计算，删除前内容会保留在审计日志中。`)) return;
    setDeletingTargetId(target.id);
    try {
      await api<void>(`/targets/${target.id}`, { method: "DELETE" });
      await onChanged("目标配置已删除，原内容已写入审计日志");
    } catch (error) { alert(errorMessage(error)); }
    finally { setDeletingTargetId(null); }
  };
  const deleteDecision = async (decision: Decision) => {
    if (!window.confirm(`确认删除决策日志“${decision.action}”吗？\n\n删除前的理由、风险和结果会保留在审计日志中。`)) return;
    setDeletingDecisionId(decision.id);
    try {
      await api<void>(`/decisions/${decision.id}`, { method: "DELETE" });
      await onChanged("决策日志已删除，原内容已写入审计日志");
    } catch (error) { alert(errorMessage(error)); }
    finally { setDeletingDecisionId(null); }
  };

  return <div className="page-stack">
    <section className="policy-hero"><div><p>投资政策声明 · IPS</p><h2>{data.policy.objective || "尚未填写投资目标"}</h2><span>期限 {data.policy.horizon_years} 年 · 最大回撤 {percent(data.policy.max_drawdown)} · 单一标的上限 {percent(data.policy.max_single_position)}</span></div><button onClick={() => open("target")}>＋ 配置目标</button></section>
    <div className="two-column planning-columns">
      <section className="card planning-card"><div className="section-head"><div><p>目标配置</p><h3>{visibleTargets.length === data.targets.length ? `${data.targets.length} 项规则` : `找到 ${visibleTargets.length} / ${data.targets.length} 项`}</h3></div><button className="link-btn" onClick={() => open("target")}>新增</button></div><div className="planning-search"><label><span>查找目标配置</span><input value={targetQuery} onChange={(event) => setTargetQuery(event.target.value)} placeholder="名称或配置维度" /></label><label><span>配置维度</span><select value={targetDimension} onChange={(event) => setTargetDimension(event.target.value)}><option value="all">全部维度</option>{targetDimensions.map((dimension) => <option key={dimension} value={dimension}>{dimensionLabels[dimension] ?? dimension}</option>)}</select></label></div><div className="target-list">{visibleTargets.map((target) => <article className="target-row" key={target.id}><button className="target-line" onClick={() => open("target", target)}><span><strong>{targetValueLabel(target)}</strong><small>{dimensionLabels[target.dimension] ?? target.dimension}</small></span><b>{percent(target.target_weight)}</b><em>{percent(target.min_weight)} – {percent(target.max_weight)}</em></button><button className="planning-delete" aria-label={`删除目标配置 ${targetValueLabel(target)}`} title="删除目标配置" disabled={deletingTargetId === target.id} onClick={() => void deleteTarget(target)}>{deletingTargetId === target.id ? "处理中" : "删除"}</button></article>)}</div>{!visibleTargets.length && <Empty title={data.targets.length ? "没有匹配的目标配置" : "尚未设置目标配置"} action={data.targets.length ? "清除查找" : "新增目标"} onAction={() => { if (data.targets.length) { setTargetQuery(""); setTargetDimension("all"); } else open("target"); }} />}</section>
      <section className="card"><div className="section-head"><div><p>周期复盘</p><h3>{data.reviews.length} 条记录</h3></div><button className="link-btn" onClick={() => open("review")}>新增</button></div>{data.reviews.slice(0, 5).map((review) => <button className="review-line" key={review.id} onClick={() => open("review", review)}><i>{review.period_type.slice(0, 1).toUpperCase()}</i><span><strong>{review.summary}</strong><small>{review.period_start} 至 {review.period_end}</small></span><b>{review.completed_at ? "已完成" : "待完成"}</b></button>)}{!data.reviews.length && <Empty title="尚未记录复盘" action="开始复盘" onAction={() => open("review")} />}</section>
    </div>
    <section className="card planning-card"><div className="section-head"><div><p>决策日志</p><h3>{decisionQuery.trim() ? `找到 ${visibleDecisions.length} / ${data.decisions.length} 条` : "过程质量与结果质量分开评价"}</h3></div><button className="primary small" onClick={() => open("decision")}>＋ 新增决策</button></div><div className="planning-search decision-search"><label><span>查找决策日志</span><input value={decisionQuery} onChange={(event) => setDecisionQuery(event.target.value)} placeholder="动作、标的、理由、风险或结果" /></label>{decisionQuery && <button type="button" onClick={() => setDecisionQuery("")}>清除</button>}</div><div className="decision-grid">{visibleDecisions.map((decision) => { const instrument = instrumentForDecision(decision); return <article className="decision-card-shell" key={decision.id}><button className="decision-card" onClick={() => open("decision", decision)}><div><i>{instrument?.symbol?.slice(0, 3) ?? "—"}</i><span>{decision.review_at && new Date(decision.review_at) < new Date() ? "待复盘" : "跟踪中"}</span></div><h3>{decision.action}</h3><p>{decision.rationale}</p><footer><span>{instrument ? `${instrument.symbol} · ` : ""}信心 {decision.confidence}%</span><span>{dateText(decision.decided_at)}</span></footer></button><button className="planning-delete decision-delete" aria-label={`删除决策日志 ${decision.action}`} title="删除决策日志" disabled={deletingDecisionId === decision.id} onClick={() => void deleteDecision(decision)}>{deletingDecisionId === decision.id ? "处理中" : "删除"}</button></article>; })}{!visibleDecisions.length && <Empty title={data.decisions.length ? "没有匹配的决策日志" : "尚未记录重要决策"} action={data.decisions.length ? "清除查找" : "记录第一条决策"} onAction={() => data.decisions.length ? setDecisionQuery("") : open("decision")} />}</div></section>
  </div>;
}

function CurrencyDataView({ data, open, onChanged }: { data: Snapshot; open: (kind: DialogKind, value?: unknown) => void; onChanged: (message: string) => Promise<void> }) {
  const [tab, setTab] = useState("accounts");
  const [refreshingFx, setRefreshingFx] = useState(false);
  const [refreshingCrypto, setRefreshingCrypto] = useState(false);
  const [runningAll, setRunningAll] = useState(false);
  const [collectorQuery, setCollectorQuery] = useState("");
  const [collectorFilter, setCollectorFilter] = useState<"all" | "prices" | "fx_rates" | "failed">("all");
  const [instrumentQuery, setInstrumentQuery] = useState("");
  const [instrumentFilter, setInstrumentFilter] = useState("all");
  const [instrumentTagFilter, setInstrumentTagFilter] = useState("all");
  const [showNetworks, setShowNetworks] = useState(false);
  const [showAllRuns, setShowAllRuns] = useState(false);
  const [testingCollectorId, setTestingCollectorId] = useState<string | null>(null);
  const [accountActionId, setAccountActionId] = useState<string | null>(null);
  const [instrumentActionId, setInstrumentActionId] = useState<string | null>(null);
  const [networkActionId, setNetworkActionId] = useState<string | null>(null);
  const coreRates = Array.from(new Map([...data.fxRates].filter((fx) => fx.base_currency === data.settings.report_currency && majorCurrencies.some((currency) => currency.code === fx.quote_currency)).sort((left, right) => new Date(left.rate_at).getTime() - new Date(right.rate_at).getTime()).map((fx) => [`${fx.base_currency}-${fx.quote_currency}`, fx])).values());
  const cryptoInstruments = data.instruments.filter((instrument) => ["crypto", "stablecoin"].includes(instrument.asset_type));
  const latestPrices = Array.from(new Map([...data.prices].sort((left, right) => new Date(left.price_at).getTime() - new Date(right.price_at).getTime()).map((price) => [price.instrument_id, price])).values());
  const cryptoUsdPrices = latestPrices.filter((price) => price.currency === "USD" && cryptoInstruments.some((instrument) => instrument.id === price.instrument_id));
  const otherMarketPrices = latestPrices.filter((price) => !cryptoInstruments.some((instrument) => instrument.id === price.instrument_id));
  const priceNeeds = Array.from(new Map(data.portfolio.holdings.filter((holding) => holding.missing_price || holding.stale).map((holding) => [holding.instrument_id, holding])).values());
  const fxNeeds = Array.from(new Set(data.portfolio.holdings.map((holding) => holding.currency).filter((currency) => currency !== data.settings.report_currency && !data.fxRates.some((fx) => (fx.base_currency === currency && fx.quote_currency === data.settings.report_currency) || (fx.quote_currency === currency && fx.base_currency === data.settings.report_currency)))));
  const enabledCollectors = data.collectors.filter((collector) => collector.is_enabled);
  const coveredInstrumentIds = new Set(data.collectors.map((collector) => typeof collector.config.instrument_id === "string" ? collector.config.instrument_id : ""));
  const uncoveredPrices = priceNeeds.filter((holding) => !coveredInstrumentIds.has(holding.instrument_id));
  const stockInstruments = data.instruments.filter((instrument) => instrument.is_active && stockMarket(instrument));
  const coveredStocks = stockInstruments.filter((instrument) => coveredInstrumentIds.has(instrument.id));
  const protectedCollectors = data.collectors.filter((collector) => collector.has_api_key);
  const customCollectors = data.collectors.filter((collector) => collector.config.provider === "generic_json");
  const failedCollectors = data.collectors.filter((collector) => collector.latest_run_status === "failed");
  const activeAccounts = data.accounts.filter((account) => account.is_active).length;
  const activeInstruments = data.instruments.filter((instrument) => instrument.is_active).length;
  const tagsForInstrument = (instrumentId: string) => data.instrumentTags.filter((tag) => tag.instrument_id === instrumentId).map((tag) => tag.name);
  const instrumentTagNames = Array.from(new Set(data.instrumentTags.map((tag) => tag.name))).sort((left, right) => left.localeCompare(right, "zh-CN"));
  const visibleInstruments = data.instruments.filter((instrument) => {
    const tags = tagsForInstrument(instrument.id);
    const matchesQuery = !instrumentQuery.trim() || `${instrument.name} ${instrument.symbol} ${tags.join(" ")}`.toLowerCase().includes(instrumentQuery.trim().toLowerCase());
    const matchesType = instrumentFilter === "all" || instrument.asset_type === instrumentFilter;
    const matchesTag = instrumentTagFilter === "all" || tags.includes(instrumentTagFilter);
    return matchesQuery && matchesType && matchesTag;
  });
  const instrumentTypes = Array.from(new Set(data.instruments.map((instrument) => instrument.asset_type)));
  const visibleRuns = showAllRuns ? data.runs : data.runs.slice(0, 8);
  const visibleCollectors = data.collectors.filter((collector) => {
    const matchesQuery = !collectorQuery.trim() || `${collector.name} ${String(collector.config.provider ?? "")} ${collector.data_type}`.toLowerCase().includes(collectorQuery.trim().toLowerCase());
    const matchesType = collectorFilter === "all" || collectorFilter === "failed" ? collectorFilter !== "failed" || collector.latest_run_status === "failed" : collector.data_type === collectorFilter;
    return matchesQuery && matchesType;
  });

  const runJob = async (job: Job) => {
    try {
      const run = await api<SyncRun>(`/sync-jobs/${job.id}/run`, { method: "POST" });
      await onChanged(run.status === "succeeded" ? "同步完成" : `同步失败：${run.error_message ?? "未知错误"}`);
    } catch (error) { alert(errorMessage(error)); }
  };
  const refreshMajorFx = async () => {
    setRefreshingFx(true);
    try {
      const result = await api<{ pairs_written: number; source: string; reference_dates: string[] }>("/fx-rates/refresh-major", { method: "POST" });
      await onChanged(`已刷新 ${result.pairs_written} 组主流汇率`);
    } catch (error) { alert(errorMessage(error)); }
    finally { setRefreshingFx(false); }
  };
  const refreshCryptoUsd = async () => {
    setRefreshingCrypto(true);
    try {
      const result = await api<{ instruments_created: number; prices_written: number; missing_symbols: string[] }>("/prices/refresh-crypto-usd", { method: "POST" });
      const missing = result.missing_symbols.length ? `；${result.missing_symbols.join("、")} 暂未返回价格` : "";
      await onChanged(`已更新 ${result.prices_written} 个币种兑美元价格，新增 ${result.instruments_created} 个标的${missing}`);
    } catch (error) { alert(errorMessage(error)); }
    finally { setRefreshingCrypto(false); }
  };
  const runAll = async () => {
    if (!enabledCollectors.length) return;
    setRunningAll(true);
    let succeeded = 0;
    try {
      for (const collector of enabledCollectors) {
        const run = await api<SyncRun>(`/api-collectors/${collector.id}/run`, { method: "POST" });
        if (run.status === "succeeded") succeeded += 1;
      }
      await onChanged(`API 更新完成：${succeeded}/${enabledCollectors.length} 个采集器成功`);
    } catch (error) { alert(errorMessage(error)); }
    finally { setRunningAll(false); }
  };
  const runCollector = async (collector: Collector) => {
    try {
      const run = await api<SyncRun>(`/api-collectors/${collector.id}/run`, { method: "POST" });
      await onChanged(run.status === "succeeded" ? `${collector.name} 获取成功` : `${collector.name} 获取失败：${run.error_message ?? "未知错误"}`);
    } catch (error) { alert(errorMessage(error)); }
  };
  const testSavedCollector = async (collector: Collector) => {
    setTestingCollectorId(collector.id);
    try {
      const result = await api<CollectorTestResult>("/api-collectors/test", { method: "POST", body: JSON.stringify({ collector_id: collector.id, name: collector.name, data_type: collector.data_type, config: collector.config }) });
      await onChanged(`连接测试成功：${providerLabel(result.provider)}，识别到 ${result.record_count} 条数据，用时 ${result.elapsed_ms} 毫秒`);
    } catch (error) { alert(errorMessage(error)); }
    finally { setTestingCollectorId(null); }
  };
  const deleteCollector = async (collector: Collector) => {
    if (!window.confirm(`确认删除采集器“${collector.name}”吗？历史运行记录会保留。`)) return;
    try { await api(`/api-collectors/${collector.id}`, { method: "DELETE" }); await onChanged("API 采集器已删除"); }
    catch (error) { alert(errorMessage(error)); }
  };
  const changeAccountStatus = async (account: Account) => {
    const nextActive = !account.is_active;
    const action = nextActive ? "解冻" : "冻结";
    if (!window.confirm(`${action}账户“${account.name}”？${nextActive ? "解冻后可以继续录入流水。" : "冻结后将停止计入组合，并禁止录入新流水；历史记录会保留。"}`)) return;
    setAccountActionId(account.id);
    try {
      await api<Account>(`/accounts/${account.id}`, { method: "PATCH", body: JSON.stringify({ is_active: nextActive }) });
      await onChanged(`账户已${action}`);
    } catch (error) { alert(errorMessage(error)); }
    finally { setAccountActionId(null); }
  };
  const deleteAccount = async (account: Account) => {
    if (!window.confirm(`永久删除账户“${account.name}”？\n\n只有没有流水和对账记录的空账户才能删除。已有历史数据的账户请使用冻结。`)) return;
    setAccountActionId(account.id);
    try {
      await api(`/accounts/${account.id}`, { method: "DELETE" });
      await onChanged("账户已删除");
    } catch (error) { alert(errorMessage(error)); }
    finally { setAccountActionId(null); }
  };
  const changeInstrumentStatus = async (instrument: Instrument) => {
    const nextActive = !instrument.is_active;
    const action = nextActive ? "恢复" : "停用";
    if (!window.confirm(`${action}标的“${instrument.symbol} · ${instrument.name}”？历史流水和价格不会被删除。`)) return;
    setInstrumentActionId(instrument.id);
    try {
      await api<Instrument>(`/instruments/${instrument.id}`, { method: "PATCH", body: JSON.stringify({ is_active: nextActive }) });
      await onChanged(`投资标的已${action}`);
    } catch (error) { alert(errorMessage(error)); }
    finally { setInstrumentActionId(null); }
  };
  const deleteInstrument = async (instrument: Instrument) => {
    if (!window.confirm(`永久删除标的“${instrument.symbol} · ${instrument.name}”？\n\n仅没有账本流水的标的可以删除；相关价格和 API 采集配置会一并清理。已有流水的标的请使用停用。`)) return;
    setInstrumentActionId(instrument.id);
    try {
      await api(`/instruments/${instrument.id}`, { method: "DELETE" });
      await onChanged("投资标的已删除");
    } catch (error) { alert(errorMessage(error)); }
    finally { setInstrumentActionId(null); }
  };
  const changeNetworkStatus = async (network: BlockchainNetwork) => {
    const nextActive = !network.is_active;
    const action = nextActive ? "恢复" : "停用";
    if (!window.confirm(`${action}网络“${network.name} · ${network.code}”？已保存到标的中的选择会保留。`)) return;
    setNetworkActionId(network.id);
    try {
      await api<BlockchainNetwork>(`/blockchain-networks/${network.id}`, { method: "PATCH", body: JSON.stringify({ is_active: nextActive }) });
      await onChanged(`区块链网络已${action}`);
    } catch (error) { alert(errorMessage(error)); }
    finally { setNetworkActionId(null); }
  };
  const deleteNetwork = async (network: BlockchainNetwork) => {
    if (!window.confirm(`永久删除网络“${network.name} · ${network.code}”？\n\n已被投资标的选择的网络不能删除，请先从相关标的中移除。`)) return;
    setNetworkActionId(network.id);
    try {
      await api(`/blockchain-networks/${network.id}`, { method: "DELETE" });
      await onChanged("区块链网络已删除");
    } catch (error) { alert(errorMessage(error)); }
    finally { setNetworkActionId(null); }
  };

  const dataTabs = [
    { id: "accounts", icon: "账", label: "账户", description: "管理资金归属与账户边界", meta: `${activeAccounts} 个启用` },
    { id: "instruments", icon: "标", label: "投资标的", description: "维护资产名称、分类与托管位置", meta: `${activeInstruments} 个有效` },
    { id: "market", icon: "价", label: "市场数据", description: "查看最新价格和参考汇率", meta: `${latestPrices.length + coreRates.length} 项最新数据` },
    { id: "sync", icon: "采", label: "自动采集", description: "配置接口、测试连接与定时更新", meta: `${enabledCollectors.length} 个运行中` },
  ];
  const instrumentLocation = (instrument: Instrument) => {
    const names = splitNetworkCodes(instrument.network).map((code) => data.networks.find((network) => network.code.toLowerCase() === code)?.name ?? code);
    return names.join("、") || instrument.exchange || "未设置";
  };

  return <div className="page-stack">
    <section className="data-workspace card">
      <div className="data-workspace-copy"><p>数据中心</p><h2>把投资资料整理成容易阅读的信息</h2><span>名称与状态优先展示，代码和技术字段仅作为辅助信息。</span></div>
      <nav className="data-section-nav" aria-label="数据页面分类">{dataTabs.map((item) => <button key={item.id} className={tab === item.id ? "active" : ""} aria-current={tab === item.id ? "page" : undefined} onClick={() => setTab(item.id)}><i>{item.icon}</i><span><strong>{item.label}</strong><small>{item.description}</small></span><b>{item.meta}</b></button>)}</nav>
    </section>
    {tab === "accounts" && <section className="card data-content-card">
      <div className="section-head"><div><p>账户管理</p><h3>资金放在哪里，一眼即可看清</h3><span className="section-description">{activeAccounts} 个账户正在计入组合，{data.accounts.length - activeAccounts} 个账户已冻结。</span></div><button className="primary small" onClick={() => open("account")}>＋ 新增账户</button></div>
      <div className="account-guidance"><i>i</i><span><strong>冻结会保留历史，删除仅适用于空账户</strong><small>冻结账户不计入当前组合，也不能录入新流水；需要时可以随时解冻。</small></span></div>
      <div className="data-table account-table"><div className="data-head"><span>账户名称</span><span>用途</span><span>计价方式</span><span>状态</span><span>操作</span></div>{data.accounts.map((account) => <div className={account.is_active ? "" : "frozen-row"} key={account.id}><span><strong>{account.name}</strong><small>{account.institution || "个人账户"}</small></span><span>{accountLabels[account.account_type] ?? account.account_type}</span><span><strong>{currencyName(account.base_currency)}</strong><small>基础币种</small></span><span className={account.is_active ? "account-status active" : "account-status frozen"}><i />{account.is_active ? "正常使用" : "已冻结"}</span><div className="account-actions"><button onClick={() => open("account", account)} disabled={accountActionId === account.id}>编辑</button><button className={account.is_active ? "freeze" : "unfreeze"} onClick={() => void changeAccountStatus(account)} disabled={accountActionId === account.id}>{accountActionId === account.id ? "处理中…" : account.is_active ? "冻结" : "解冻"}</button><button className="delete" onClick={() => void deleteAccount(account)} disabled={accountActionId === account.id}>删除</button></div></div>)}</div>
    </section>}
    {tab === "instruments" && <div className="page-stack">
      <section className="card data-content-card">
        <div className="section-head"><div><p>投资标的</p><h3>先看名称，再看代码</h3><span className="section-description">股票、基金、现金和数字资产都在这里统一维护。</span></div><button className="primary small" onClick={() => open("instrument")}>＋ 新增标的</button></div>
        <div className="account-guidance"><i>i</i><span><strong>有历史流水的标的请停用，不要删除</strong><small>停用会保留持仓、价格和审计记录；永久删除仅适用于尚未产生账本流水的空标的。</small></span></div>
        <div className="entity-toolbar instrument-toolbar"><label><span>搜索标的或标签</span><input value={instrumentQuery} onChange={(event) => setInstrumentQuery(event.target.value)} placeholder="名称、代码或标签" /></label><label><span>资产类别</span><select value={instrumentFilter} onChange={(event) => setInstrumentFilter(event.target.value)}><option value="all">全部类别</option>{instrumentTypes.map((type) => <option key={type} value={type}>{assetLabels[type] ?? type}</option>)}</select></label><label><span>类别标签</span><select value={instrumentTagFilter} onChange={(event) => setInstrumentTagFilter(event.target.value)}><option value="all">全部标签</option>{instrumentTagNames.map((tag) => <option key={tag} value={tag}>{tag}</option>)}</select></label><button className={showNetworks ? "soft compact active" : "soft compact"} onClick={() => setShowNetworks((current) => !current)}>{showNetworks ? "收起网络设置" : `管理区块链网络（${data.networks.length}）`}</button></div>
        <div className="data-table instrument-table"><div className="data-head"><span>名称</span><span>类别与标签</span><span>计价方式</span><span>交易或托管位置</span><span>状态</span><span>操作</span></div>{visibleInstruments.map((instrument) => { const tags = tagsForInstrument(instrument.id); return <div className={instrument.is_active ? "" : "frozen-row"} key={instrument.id}><span><strong>{instrument.name}</strong><small><em className="entity-code">{instrument.symbol}</em></small></span><span className="instrument-classification"><strong>{assetLabels[instrument.asset_type] ?? instrument.asset_type}</strong><small className="instrument-tag-list">{tags.length ? tags.map((tag) => <em className="instrument-tag-badge" key={tag}>{tag}</em>) : <em className="instrument-tag-empty">未设置标签</em>}</small></span><span>{currencyName(instrument.currency)}</span><span>{instrumentLocation(instrument)}</span><span className={instrument.is_active ? "account-status active" : "account-status frozen"}><i />{instrument.is_active ? "正常使用" : "已停用"}</span><div className="account-actions"><button onClick={() => open("instrument", instrument)} disabled={instrumentActionId === instrument.id}>编辑</button><button className={instrument.is_active ? "freeze" : "unfreeze"} onClick={() => void changeInstrumentStatus(instrument)} disabled={instrumentActionId === instrument.id}>{instrumentActionId === instrument.id ? "处理中…" : instrument.is_active ? "停用" : "恢复"}</button><button className="delete" onClick={() => void deleteInstrument(instrument)} disabled={instrumentActionId === instrument.id}>删除</button></div></div>; })}</div>
        {!visibleInstruments.length && <Empty title="没有符合条件的投资标的" action="清除筛选" onAction={() => { setInstrumentQuery(""); setInstrumentFilter("all"); setInstrumentTagFilter("all"); }} />}
      </section>
      {showNetworks && <section className="card network-manager-card">
        <div className="section-head"><div><p>区块链网络</p><h3>数字资产可选择的托管网络</h3><span className="section-description">{data.networks.filter((network) => network.is_active).length} 个网络可用，代码仅在名称下方辅助显示。</span></div><button className="primary small" onClick={() => open("network")}>＋ 新增网络</button></div>
        <div className="data-table network-table"><div className="data-head"><span>网络名称</span><span>使用情况</span><span>状态</span><span>操作</span></div>{data.networks.map((network) => { const used = data.instruments.filter((instrument) => splitNetworkCodes(instrument.network).includes(network.code.toLowerCase())).length; return <div className={network.is_active ? "" : "frozen-row"} key={network.id}><span><strong>{network.name}</strong><small><em className="entity-code">{network.code}</em></small></span><span>{used ? `已用于 ${used} 个投资标的` : "尚未使用"}</span><span className={network.is_active ? "account-status active" : "account-status frozen"}><i />{network.is_active ? "可以选择" : "已停用"}</span><div className="account-actions"><button onClick={() => open("network", network)} disabled={networkActionId === network.id}>编辑</button><button className={network.is_active ? "freeze" : "unfreeze"} onClick={() => void changeNetworkStatus(network)} disabled={networkActionId === network.id}>{networkActionId === network.id ? "处理中…" : network.is_active ? "停用" : "恢复"}</button><button className="delete" onClick={() => void deleteNetwork(network)} disabled={networkActionId === network.id}>删除</button></div></div>; })}</div>
      </section>
      }
    </div>}
    {tab === "market" && <div className="page-stack">
      <div className="two-column">
        <section className="card data-content-card">
          <div className="section-head"><div><p>最新行情</p><h3>股票、基金与其他资产</h3><span className="section-description">每个标的只展示最新一条价格。</span></div><button className="link-btn" onClick={() => open("price")}>手动录入</button></div>
          {otherMarketPrices.map((price) => { const instrument = data.instruments.find((item) => item.id === price.instrument_id); return <button className="market-line readable-market-line" key={`${price.instrument_id}-${price.source}`} onClick={() => open("price", price)}><i>{(assetLabels[instrument?.asset_type ?? ""] ?? "资").slice(0, 1)}</i><span><strong>{instrument?.name ?? "未知标的"}</strong><small><em className="entity-code">{instrument?.symbol ?? "?"}</em>{sourceLabel(price.source)} · {dateText(price.price_at)}</small></span><b><small>{currencyName(price.currency)}</small>{compactNumber(price.price)}</b></button>; })}
          {!otherMarketPrices.length && <Empty title="尚无其他市场价格" action="录入价格" onAction={() => open("price")} />}
        </section>
        <section className="card data-content-card">
          <div className="section-head"><div><p>参考汇率</p><h3>以{currencyName(data.settings.report_currency)}为基准</h3><span className="section-description">显示最近发布的主流货币换算关系。</span></div><div className="section-actions"><button className="link-btn" disabled={refreshingFx} onClick={() => void refreshMajorFx()}>{refreshingFx ? "刷新中…" : "刷新"}</button><button className="link-btn" onClick={() => open("fx")}>手动录入</button></div></div>
          {coreRates.map((fx) => <button className="market-line readable-market-line" key={`${fx.base_currency}-${fx.quote_currency}`} onClick={() => open("fx", fx)}><i>汇</i><span><strong>{currencyName(fx.base_currency)}兑换{currencyName(fx.quote_currency)}</strong><small>{sourceLabel(fx.source)} · {dateText(fx.rate_at)}</small></span><b><small>参考汇率</small>{compactNumber(fx.rate)}</b></button>)}
          {!coreRates.length && <Empty title="尚无当前核心币种汇率" action="立即刷新" onAction={() => void refreshMajorFx()} />}
        </section>
      </div>
      <section className="card crypto-usd-card data-content-card">
        <div className="section-head"><div><p>数字资产</p><h3>虚拟货币与稳定币兑美元</h3><span className="section-description">{cryptoUsdPrices.length} 个币种已有最新价格。</span></div><div className="section-actions"><button className="primary small" disabled={refreshingCrypto} onClick={() => void refreshCryptoUsd()}>{refreshingCrypto ? "获取中…" : "刷新主流币种"}</button><button className="link-btn" onClick={() => open("price")}>手动录入</button></div></div>
        <div className="crypto-rate-grid">{cryptoUsdPrices.map((price) => { const instrument = data.instruments.find((item) => item.id === price.instrument_id); return <button key={`${price.instrument_id}-${price.source}`} onClick={() => open("price", price)}><i>{instrument?.asset_type === "stablecoin" ? "稳" : "币"}</i><span><strong>{instrument?.name ?? "未知数字资产"}</strong><small><em className="entity-code">{instrument?.symbol ?? "?"}</em>{instrument ? assetLabels[instrument.asset_type] : "虚拟货币"} · 美元计价</small></span><b>{compactNumber(price.price)}</b></button>; })}</div>
        {!cryptoUsdPrices.length && <Empty title="尚无虚拟货币兑美元价格" action="获取主流币种" onAction={() => void refreshCryptoUsd()} />}
        <p className="note">刷新会更新主流数字资产的公开美元价格，不会连接交易所账户或执行交易。</p>
      </section>
    </div>}
    {tab === "sync" && <div className="page-stack">
      <section className="api-manager card">
        <div className="api-manager-head"><div><p>自动采集</p><h2>数据采集与更新</h2><span>集中管理外部接口、访问密钥、连接测试和定时更新。</span></div><div className="section-actions"><button className="soft compact" disabled={runningAll || !enabledCollectors.length} onClick={() => void runAll()}>{runningAll ? "正在更新…" : `更新全部（${enabledCollectors.length}）`}</button><button className="primary" onClick={() => open("api-manager")}>＋ 新增采集器</button></div></div>
        <div className="api-capability-grid"><article><i>接</i><span><strong>自定义接口</strong><small>{customCollectors.length} 个接口支持字段映射</small></span></article><article><i>密</i><span><strong>安全凭据</strong><small>{protectedCollectors.length} 个采集器已保存访问密钥</small></span></article><article><i>验</i><span><strong>连接测试</strong><small>写入账本前先验证返回数据</small></span></article><article><i>定</i><span><strong>定时更新</strong><small>{enabledCollectors.length} 个采集器正在自动运行</small></span></article></div>
        <div className="market-auto-note"><i>↻</i><span><strong>美股、港股行情自动更新</strong><small>后台每 5 分钟获取一次，已覆盖 {coveredStocks.length} 个标的，共 {stockInstruments.length} 个股票与 ETF。</small></span></div>
        <div className="api-health-grid"><div><i className={uncoveredPrices.length ? "warn" : "ok"}>{uncoveredPrices.length ? "!" : "✓"}</i><span><small>未配置价格源</small><strong>{uncoveredPrices.length}</strong></span></div><div><i className={fxNeeds.length ? "warn" : "ok"}>{fxNeeds.length ? "!" : "✓"}</i><span><small>缺失汇率对</small><strong>{fxNeeds.length}</strong></span></div><div><i className="ok">↻</i><span><small>启用采集器</small><strong>{enabledCollectors.length}</strong></span></div><div><i className={failedCollectors.length ? "warn" : "ok"}>{failedCollectors.length ? "!" : "✓"}</i><span><small>运行失败</small><strong>{failedCollectors.length}</strong></span></div></div>
        <div className="data-needs">
          <div><h3>需要补充价格</h3>{priceNeeds.length ? priceNeeds.map((holding) => <button key={holding.instrument_id} onClick={() => open("api-manager", { mode: "price", instrument_id: holding.instrument_id } satisfies ApiManagerSeed)}><span><strong>{holding.name}</strong><small><em className="entity-code">{holding.symbol}</em>{holding.missing_price ? "缺少价格" : "价格需要更新"} · {currencyName(holding.currency)}计价</small></span><b>{coveredInstrumentIds.has(holding.instrument_id) ? "已配置" : "配置采集 →"}</b></button>) : <p>✓ 所有持仓价格均为有效状态</p>}</div>
          <div><h3>需要补充汇率</h3>{fxNeeds.length ? fxNeeds.map((currency) => <button key={currency} onClick={() => open("api-manager", { mode: "fx", base: currency, quote: data.settings.report_currency } satisfies ApiManagerSeed)}><span><strong>{currencyName(currency)}兑换{currencyName(data.settings.report_currency)}</strong><small>组合估值需要这项换算关系</small></span><b>配置采集 →</b></button>) : <p>✓ 当前组合所需汇率已完整覆盖</p>}</div>
        </div>
      </section>
      <section className="card collector-registry">
        <div className="section-head"><div><p>采集器</p><h3>自动更新规则</h3><span className="section-description">当前显示 {visibleCollectors.length} 个，共 {data.collectors.length} 个。</span></div><button className="primary small" onClick={() => open("api-manager")}>＋ 新增</button></div>
        <div className="collector-toolbar"><input value={collectorQuery} onChange={(event) => setCollectorQuery(event.target.value)} placeholder="按采集器名称搜索" /><select value={collectorFilter} onChange={(event) => setCollectorFilter(event.target.value as typeof collectorFilter)}><option value="all">全部采集器</option><option value="prices">市场价格</option><option value="fx_rates">外汇汇率</option><option value="failed">需要处理</option></select></div>
        <div className="collector-card-list">{visibleCollectors.map((collector) => <article key={collector.id}><button className="collector-name" onClick={() => open("api-manager", collector)}><i>{collector.data_type === "fx_rates" ? "汇" : "价"}</i><span><strong>{collector.name}</strong><small>{providerLabel(collector.config.provider ?? collector.source_type)}{collector.has_api_key ? " · 已保存访问密钥" : ""}</small></span></button><div className="collector-card-facts"><span><small>采集内容</small><strong>{collector.data_type === "fx_rates" ? "外汇汇率" : "市场价格"}</strong></span><span><small>更新计划</small><strong>{intervalLabel(collector.interval_seconds)}</strong></span><span><small>最近状态</small><strong className={collector.latest_run_status === "failed" ? "negative" : collector.latest_run_status === "succeeded" ? "positive" : ""}>{collector.latest_run_status === "succeeded" ? "获取成功" : collector.latest_run_status === "failed" ? "需要处理" : collector.latest_run_status === "running" ? "正在获取" : "尚未运行"}</strong></span><span><small>自动更新</small><strong className={collector.is_enabled ? "status-ok" : "status-off"}>{collector.is_enabled ? "已开启" : "已暂停"}</strong></span></div><div className="collector-actions"><button onClick={() => open("api-manager", collector)}>编辑</button><button disabled={testingCollectorId === collector.id} onClick={() => void testSavedCollector(collector)}>{testingCollectorId === collector.id ? "测试中" : "测试连接"}</button><button onClick={() => void runCollector(collector)}>立即获取</button><button className="delete" onClick={() => void deleteCollector(collector)}>删除</button></div></article>)}</div>
        {!visibleCollectors.length && <Empty title={data.collectors.length ? "没有匹配的 API 采集器" : "尚未创建 API 采集器"} action="新增采集器" onAction={() => open("api-manager")} />}
      </section>
      <details className="card data-advanced"><summary><span><strong>高级数据源与任务设置</strong><small>日常使用无需展开；这里保留底层数据源和调度任务的独立配置。</small></span><b>展开查看</b></summary><div className="two-column">
        <section><div className="section-head"><div><p>数据来源</p><h3>{data.sources.length} 个来源</h3></div><button className="link-btn" onClick={() => open("source")}>新增来源</button></div>{data.sources.map((source) => <button className="sync-line" key={source.id} onClick={() => open("source", source)}><i className={source.is_enabled ? "on" : ""}>↻</i><span><strong>{source.name}</strong><small>{providerLabel(source.config.provider ?? source.source_type)}</small></span><b>{source.is_enabled ? "正常使用" : "已暂停"}</b></button>)}{!data.sources.length && <Empty title="尚未配置数据源" action="新增采集器" onAction={() => open("api-manager")} />}</section>
        <section><div className="section-head"><div><p>定时任务</p><h3>{data.jobs.length} 个任务</h3></div><button className="link-btn" onClick={() => open("job")}>新增任务</button></div>{data.jobs.map((job) => { const latest = data.runs.find((run) => run.job_id === job.id); return <div className="sync-line" key={job.id}><i className={latest?.status === "failed" ? "failed" : job.is_enabled ? "on" : ""}>{latest?.status === "failed" ? "!" : "◷"}</i><button className="sync-copy" onClick={() => open("job", job)}><strong>{job.name}</strong><small>{latest ? `上次${latest.status === "succeeded" ? "成功" : latest.status === "failed" ? "失败" : "正在运行"} · ${dateText(latest.started_at)}` : "尚未运行"} · {intervalLabel(job.interval_seconds)}</small></button><button className="run-btn" onClick={() => void runJob(job)}>立即获取</button></div>; })}{!data.jobs.length && <Empty title="尚未建立定时任务" action="新增采集器" onAction={() => open("api-manager")} />}</section>
      </div></details>
      <section className="card run-history-card"><div className="section-head"><div><p>更新记录</p><h3>最近运行情况</h3><span className="section-description">只展示易读结果，不显示内部编号和原始返回内容。</span></div>{data.runs.length > 8 && <button className="link-btn" onClick={() => setShowAllRuns((current) => !current)}>{showAllRuns ? "收起" : `查看全部（${data.runs.length}）`}</button>}</div><div className="run-list">{visibleRuns.map((run) => <div key={run.id}><i className={run.status}>{run.status === "succeeded" ? "✓" : run.status === "running" ? "…" : "!"}</i><span><strong>{data.jobs.find((job) => job.id === run.job_id)?.name ?? "后台数据采集"}</strong><small>{runSummary(run)}</small></span><b>{dateText(run.started_at)}</b></div>)}</div>{!visibleRuns.length && <Empty title="尚无更新记录" action="立即更新" onAction={() => void runAll()} />}</section>
    </div>}
  </div>;
}

function SettingsView({ data, user, appearance, onAppearanceChange, onSaved, onLogout }: { data: Snapshot; user: User; appearance: Appearance; onAppearanceChange: (appearance: Appearance) => void; onSaved: (message: string) => Promise<void>; onLogout: () => Promise<void> }) {
  return <div className="settings-grid">
    <AppearanceSettings value={appearance} onChange={onAppearanceChange} />
    <CurrencySettingsForm value={data.settings} onSaved={onSaved} />
    <NetworkProxyForm value={data.networkProxy} onSaved={onSaved} />
    <PolicyForm value={data.policy} onSaved={onSaved} />
    <section className="card security-card"><div className="section-head"><div><p>安全</p><h3>本地登录</h3></div></div><div className="security-user"><b>{user.display_name.slice(0, 1)}</b><span><strong>{user.display_name}</strong><small>@{user.username} · 会话受 HttpOnly Cookie 保护</small></span></div><p>服务仅监听 127.0.0.1。券商与交易所只读密钥不应直接写入数据库，请使用系统安全凭据引用。</p><button className="danger" onClick={() => void onLogout()}>退出登录</button></section>
    <AuditExportCard />
  </div>;
}

function AuditExportCard() {
  const [preset, setPreset] = useState<AuditPreset>("all");
  const [from, setFrom] = useState("");
  const [to, setTo] = useState("");
  const [includeMarket, setIncludeMarket] = useState(true);
  const [summary, setSummary] = useState<AuditSummary | null>(null);
  const [loadingSummary, setLoadingSummary] = useState(true);
  const [downloading, setDownloading] = useState<string | null>(null);
  const [error, setError] = useState("");

  const makeQuery = useCallback((withMarket = includeMarket) => {
    const parameters = new URLSearchParams();
    if (from) parameters.set("from", from);
    if (to) parameters.set("to", to);
    parameters.set("include_market", String(withMarket));
    return `?${parameters.toString()}`;
  }, [from, includeMarket, to]);

  const refreshSummary = useCallback(async () => {
    if (from && to && from > to) { setError("开始日期不能晚于结束日期"); setSummary(null); setLoadingSummary(false); return; }
    setLoadingSummary(true); setError("");
    try { setSummary(await api<AuditSummary>(`/audit-export/summary${makeQuery(false)}`)); }
    catch (failure) { setError(errorMessage(failure)); }
    finally { setLoadingSummary(false); }
  }, [from, makeQuery, to]);

  useEffect(() => { const timer = window.setTimeout(() => { void refreshSummary(); }, 180); return () => window.clearTimeout(timer); }, [refreshSummary]);

  const choosePreset = (next: AuditPreset) => {
    const today = new Date();
    const localIso = (date: Date) => new Date(date.getTime() - date.getTimezoneOffset() * 60_000).toISOString().slice(0, 10);
    setPreset(next);
    if (next === "all") { setFrom(""); setTo(""); return; }
    if (next === "year") { setFrom(`${today.getFullYear()}-01-01`); setTo(localIso(today)); return; }
    if (next === "12m") { const start = new Date(today); start.setFullYear(start.getFullYear() - 1); setFrom(localIso(start)); setTo(localIso(today)); }
  };

  const download = async (kind: string, endpoint: string, fallback: string) => {
    if (from && to && from > to) { setError("开始日期不能晚于结束日期"); return; }
    setDownloading(kind); setError("");
    try { await downloadApiFile(`/audit-export/${endpoint}${makeQuery()}`, fallback); }
    catch (failure) { setError(errorMessage(failure)); }
    finally { setDownloading(null); }
  };

  const countMetrics = summary ? [
    ["账本流水", summary.counts.transactions], ["复式分录", summary.counts.transaction_legs], ["变更记录", summary.counts.audit_logs],
    ["对账记录", summary.counts.reconciliations], ["行情与汇率", summary.counts.prices + summary.counts.fx_rates],
  ] as const : [];
  const exportDate = new Date().toISOString().slice(0, 10);
  const downloads = [
    { id: "package", endpoint: "package", extension: "json", title: "完整审计包", detail: "清单、控制总数、完整性检查、业务主数据与 SHA-256 校验" },
    { id: "transactions", endpoint: "transactions.csv", extension: "csv", title: "流水分录明细", detail: "逐条展开账户、标的、数量、单价、来源及冲销关系" },
    { id: "changes", endpoint: "changes.csv", extension: "csv", title: "数据变更日志", detail: "记录实体、动作、修改前后内容与发生时间" },
    { id: "reconciliations", endpoint: "reconciliations.csv", extension: "csv", title: "账户对账明细", detail: "机构余额、账本余额、差异与对账备注" },
  ] as const;

  return <section className="card audit-export-card">
    <div className="audit-export-head"><div><p>数据审计</p><h3>审计导出中心</h3><span>直接从本地数据库生成，不受账本页面 200 条展示上限影响。</span></div><b className={summary?.integrity.passed ? "passed" : summary ? "review" : "pending"}>{loadingSummary ? "检查中" : summary?.integrity.passed ? "结构完整" : summary ? "需要复核" : "等待检查"}</b></div>
    <div className="audit-privacy-note"><i>✓</i><span><strong>安全导出</strong><small>不会包含登录密码、登录会话或 API 密钥；数据源只标记是否配置了凭据。</small></span></div>

    <div className="audit-controls">
      <div className="audit-presets" aria-label="审计时间范围">
        {([
          { id: "all", label: "全部期间" },
          { id: "year", label: "本年度" },
          { id: "12m", label: "近 12 个月" },
          { id: "custom", label: "自定义" },
        ] as const).map((item) => <button key={item.id} type="button" className={preset === item.id ? "active" : ""} onClick={() => choosePreset(item.id)}>{item.label}</button>)}
      </div>
      <div className="audit-range">
        <label>开始日期<input type="date" value={from} onChange={(event) => { setFrom(event.target.value); setPreset("custom"); }} /></label>
        <label>结束日期<input type="date" value={to} onChange={(event) => { setTo(event.target.value); setPreset("custom"); }} /></label>
        <label className="audit-market-toggle"><input type="checkbox" checked={includeMarket} onChange={(event) => setIncludeMarket(event.target.checked)} /><span><strong>包含历史行情</strong><small>完整审计包附带价格和汇率记录</small></span></label>
        <button type="button" className="soft audit-refresh" disabled={loadingSummary} onClick={() => void refreshSummary()}>{loadingSummary ? "正在检查…" : "刷新审计检查"}</button>
      </div>
    </div>

    {error && <div className="form-error audit-error">{error}</div>}
    {summary && <>
      <div className="audit-metrics">{countMetrics.map(([label, value]) => <span key={label}><small>{label}</small><strong>{value.toLocaleString("zh-CN")}</strong></span>)}</div>
      <div className="audit-check-section">
        <div className="audit-subhead"><div><strong>数据完整性检查</strong><small>关键异常会标记为“需要复核”，对账提醒不会阻止导出。</small></div><span>{summary.integrity.critical_issue_count} 个关键异常 · {summary.integrity.warning_count} 个提醒</span></div>
        <div className="audit-checks">{summary.integrity.checks.map((check) => <div key={check.code} className={check.count === 0 ? "clean" : check.level}><i>{check.count === 0 ? "✓" : "!"}</i><span><strong>{check.label}</strong><small>{check.detail}</small></span><b>{check.count === 0 ? "正常" : String(check.count) + " 项"}</b></div>)}</div>
      </div>
    </>}

    <div className="audit-download-grid">
      {downloads.map((item) => <article key={item.id}><div><i>{item.extension.toUpperCase()}</i><span><strong>{item.title}</strong><small>{item.detail}</small></span></div><button type="button" className={item.id === "package" ? "primary" : "soft"} disabled={!summary || loadingSummary || downloading !== null} onClick={() => void download(item.id, item.endpoint, "sanyu-audit-" + item.id + "-" + exportDate + "." + item.extension)}>{downloading === item.id ? "正在生成…" : "导出文件"}</button></article>)}
    </div>
    <footer className="audit-footer"><span>口径版本 {summary?.schema_version ?? "—"} · 核心币种 {summary?.report_currency ?? "—"} · 生成于 {summary ? dateText(summary.generated_at) : "—"}</span><small>SQLite 主库仍建议在停止服务后定期复制，并实际验证恢复流程。</small></footer>
  </section>;
}

function EditorDialog({ dialog, data, close, saved }: { dialog: { kind: DialogKind; value?: unknown }; data: Snapshot; close: () => void; saved: (message: string) => Promise<void> }) {
  const titleId = useId();
  const modalRef = useRef<HTMLElement>(null);
  const closeButtonRef = useRef<HTMLButtonElement>(null);
  const closeRef = useRef(close);
  useEffect(() => { closeRef.current = close; }, [close]);
  useEffect(() => {
    const previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    closeButtonRef.current?.focus();
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        closeRef.current();
        return;
      }
      if (event.key !== "Tab" || !modalRef.current) return;
      const controls = Array.from(modalRef.current.querySelectorAll<HTMLElement>('button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [href], [tabindex]:not([tabindex="-1"])'));
      if (!controls.length) return;
      const first = controls[0];
      const last = controls[controls.length - 1];
      if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus(); }
      else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
    };
    document.addEventListener("keydown", handleKeyDown);
    return () => { document.removeEventListener("keydown", handleKeyDown); previousFocus?.focus(); };
  }, []);
  const titles: Record<DialogKind, string> = { account: "账户", instrument: "投资标的", network: "区块链网络", transaction: "账本流水", "transaction-detail": "流水详情", price: "市场价格", fx: "外汇汇率", target: "配置目标", decision: "决策记录", review: "复盘记录", source: "API 数据源", job: "同步任务", "api-manager": dialog.value && typeof dialog.value === "object" && "id" in dialog.value ? "编辑 API 采集器" : "新增 API 采集器" };
  return <div className="modal-backdrop"><section ref={modalRef} className={`modal ${dialog.kind === "api-manager" ? "api-manager-modal" : ""}`} role="dialog" aria-modal="true" aria-labelledby={titleId}><header><div><p>{dialog.kind === "api-manager" ? "数据管理器" : dialog.kind === "transaction-detail" ? "查看" : dialog.value ? "编辑" : "新增"}</p><h2 id={titleId}>{titles[dialog.kind]}</h2></div><button ref={closeButtonRef} type="button" aria-label="关闭弹窗" title="关闭" onClick={close}>×</button></header>{dialog.kind === "account" && <AccountForm value={dialog.value as Account | undefined} saved={saved} />}{dialog.kind === "instrument" && <InstrumentForm value={dialog.value as Instrument | undefined} networks={data.networks} tags={data.instrumentTags} saved={saved} />}{dialog.kind === "network" && <NetworkForm value={dialog.value as BlockchainNetwork | undefined} saved={saved} />}{dialog.kind === "transaction" && <TransactionForm value={dialog.value as Transaction | undefined} data={data} saved={saved} />}{dialog.kind === "transaction-detail" && <TransactionDetail value={dialog.value as Transaction} data={data} />}{dialog.kind === "price" && <PriceForm value={dialog.value as Price | undefined} instruments={data.instruments} saved={saved} />}{dialog.kind === "fx" && <FxForm value={dialog.value as Fx | undefined} saved={saved} />}{dialog.kind === "target" && <TargetForm value={dialog.value as Target | undefined} saved={saved} />}{dialog.kind === "decision" && <DecisionForm value={dialog.value as Decision | undefined} instruments={data.instruments} saved={saved} />}{dialog.kind === "review" && <ReviewForm value={dialog.value as Review | undefined} saved={saved} />}{dialog.kind === "source" && <SourceForm value={dialog.value as Source | undefined} saved={saved} />}{dialog.kind === "job" && <JobForm value={dialog.value as Job | undefined} sources={data.sources} saved={saved} />}{dialog.kind === "api-manager" && <ApiManagerForm value={dialog.value as ApiManagerSeed | Collector | undefined} data={data} saved={saved} />}</section></div>;
}

function TransactionDetail({ value, data }: { value: Transaction; data: Snapshot }) { return <div className="transaction-detail"><div className="detail-status"><span>{transactionLabels[value.transaction_type] ?? value.transaction_type}</span><b>{value.status === "confirmed" ? "已确认" : "已冲销"}</b></div><dl><div><dt>交易时间</dt><dd>{dateText(value.trade_at)}</dd></div><div><dt>数据来源</dt><dd>{value.source}</dd></div><div><dt>外部流水号</dt><dd>{value.external_id || "—"}</dd></div><div><dt>流水 ID</dt><dd className="mono">{value.id}</dd></div><div className="wide"><dt>备注</dt><dd>{value.memo || "无备注"}</dd></div></dl><section><div className="legs-head"><strong>完整分录</strong><span>{value.legs.length} 条</span></div>{value.legs.map((leg, index) => <div className="detail-leg" key={index}><i>{index + 1}</i><span><strong>{data.instruments.find((item) => item.id === leg.instrument_id)?.symbol ?? "未知标的"}</strong><small>{data.accounts.find((item) => item.id === leg.account_id)?.name ?? "未知账户"} · {leg.leg_type}</small></span><b className={number(leg.quantity) >= 0 ? "positive" : "negative"}>{number(leg.quantity) > 0 ? "+" : ""}{leg.quantity}</b><em>{leg.unit_price ? `@ ${leg.unit_price} ${leg.price_currency ?? ""}` : "—"}</em></div>)}</section></div>; }

function FormShell({ children, submit, label }: { children: ReactNode; submit: (event: FormEvent) => Promise<void>; label: string }) { const [busy, setBusy] = useState(false); const [error, setError] = useState(""); return <form onSubmit={async (event) => { event.preventDefault(); setBusy(true); setError(""); try { await submit(event); } catch (failure) { setError(errorMessage(failure)); } finally { setBusy(false); } }}>{children}{error && <div className="form-error">{error}</div>}<footer><button type="submit" className="primary" disabled={busy}>{busy ? "正在保存…" : label}</button></footer></form>; }

function MajorCurrencySelect({ value, onChange }: { value: string; onChange: (value: string) => void }) { return <select value={normalizeMajorCurrency(value)} onChange={(event) => onChange(event.target.value)}>{majorCurrencies.map((currency) => <option key={currency.code} value={currency.code}>{currency.code} · {currency.label}</option>)}</select>; }

function MultiChoice({ label, values, options, onToggle, help }: { label: string; values: string[]; options: string[]; onToggle: (value: string) => void; help: string }) { return <fieldset className="multi-choice"><legend>{label}</legend><div>{options.map((option) => { const selected = values.includes(option); return <button key={option} type="button" className={selected ? "active" : ""} aria-pressed={selected} onClick={() => onToggle(option)}><i>{selected ? "✓" : "+"}</i>{option}</button>; })}</div><small>{help}</small></fieldset>; }

function NetworkMultiChoice({ networks, values, onToggle }: { networks: BlockchainNetwork[]; values: string[]; onToggle: (code: string) => void }) { const known = new Set(networks.map((network) => network.code.toLowerCase())); const options = [...networks.filter((network) => network.is_active || values.includes(network.code.toLowerCase())), ...values.filter((code) => !known.has(code)).map((code) => ({ id: `legacy-${code}`, code, name: code, is_active: false, created_at: "", updated_at: "" }))]; return <fieldset className="multi-choice network-choice"><legend>区块链网络（可多选）</legend><div>{options.map((network) => { const code = network.code.toLowerCase(); const selected = values.includes(code); return <button key={network.id} type="button" className={selected ? "active" : ""} aria-pressed={selected} onClick={() => onToggle(code)}><i>{selected ? "✓" : "+"}</i><span>{network.name}<small>{network.code}{network.is_active ? "" : " · 已停用"}</small></span></button>; })}</div><small>{options.length ? "可选择多个网络；选项可在“数据 → 标的 → 区块链网络字典”中维护。" : "暂无可选网络，请先在区块链网络字典中新增。"}</small></fieldset>; }

function AccountForm({ value, saved }: { value?: Account; saved: (message: string) => Promise<void> }) { const [name, setName] = useState(value?.name ?? ""); const [institution, setInstitution] = useState(value?.institution ?? ""); const [type, setType] = useState(value?.account_type ?? "brokerage"); const [currency, setCurrency] = useState(normalizeMajorCurrency(value?.base_currency)); const [included, setIncluded] = useState(value?.include_in_net_worth ?? true); return <FormShell label={value ? "保存账户" : "创建账户"} submit={async () => { await api(value ? `/accounts/${value.id}` : "/accounts", { method: value ? "PUT" : "POST", body: JSON.stringify({ name, institution: institution || null, account_type: type, base_currency: currency, include_in_net_worth: included }) }); await saved(value ? "账户已更新" : "账户已创建"); }}><label>账户名称<input value={name} onChange={(e) => setName(e.target.value)} required /></label><label>机构<input value={institution} onChange={(e) => setInstitution(e.target.value)} placeholder="可选" /></label><div className="form-grid"><label>账户类型<select value={type} onChange={(e) => setType(e.target.value)}>{Object.entries(accountLabels).map(([id, label]) => <option key={id} value={id}>{label}</option>)}</select></label><label>基础币种<MajorCurrencySelect value={currency} onChange={setCurrency} /></label></div><label className="check"><input type="checkbox" checked={included} onChange={(e) => setIncluded(e.target.checked)} />计入净资产</label></FormShell>; }

function InstrumentTagEditor({ values, suggestions, onChange }: { values: string[]; suggestions: string[]; onChange: (values: string[]) => void }) {
  const [customTag, setCustomTag] = useState("");
  const addTag = (raw: string) => {
    const tag = raw.trim();
    if (!tag || values.some((item) => item.toLowerCase() === tag.toLowerCase()) || values.length >= 20) return;
    onChange([...values, tag]);
    setCustomTag("");
  };
  const options = Array.from(new Set(["核心", "卫星", "长期持有", "定投", "成长型", "收益型", "低波动", "高风险", "高流动性", "A股", "港股", "美股", "数字资产", ...suggestions])).filter((tag) => !values.some((selected) => selected.toLowerCase() === tag.toLowerCase())).slice(0, 18);
  return <fieldset className="tag-editor"><legend>类别标签（可多选）</legend><div className="selected-tags">{values.length ? values.map((tag) => <button type="button" key={tag} title={`移除 ${tag}`} onClick={() => onChange(values.filter((item) => item !== tag))}>{tag}<i>×</i></button>) : <span>暂未设置标签</span>}</div><div className="tag-suggestions">{options.map((tag) => <button type="button" key={tag} onClick={() => addTag(tag)}>＋ {tag}</button>)}</div><div className="tag-add-row"><input value={customTag} maxLength={30} onChange={(event) => setCustomTag(event.target.value)} onKeyDown={(event) => { if (event.key === "Enter") { event.preventDefault(); addTag(customTag); } }} placeholder="输入自定义标签，如：养老、教育金" /><button type="button" onClick={() => addTag(customTag)} disabled={!customTag.trim() || values.length >= 20}>添加</button></div><small>最多 20 个标签；点击已选标签可以移除。标签会用于标的列表和新增流水筛选。</small></fieldset>;
}

function InstrumentForm({ value, networks, tags, saved }: { value?: Instrument; networks: BlockchainNetwork[]; tags: InstrumentTag[]; saved: (message: string) => Promise<void> }) {
  const [symbol, setSymbol] = useState(value?.symbol ?? "");
  const [name, setName] = useState(value?.name ?? "");
  const [type, setType] = useState(value?.asset_type ?? "etf");
  const [currency, setCurrency] = useState(normalizeMajorCurrency(value?.currency));
  const [exchange, setExchange] = useState(value?.exchange ?? "");
  const [selectedNetworks, setSelectedNetworks] = useState(splitNetworkCodes(value?.network));
  const [contract, setContract] = useState(value?.contract_address ?? "");
  const [precision, setPrecision] = useState(String(value?.precision ?? (type === "crypto" ? 8 : 4)));
  const [selectedTags, setSelectedTags] = useState(tags.filter((tag) => tag.instrument_id === value?.id).map((tag) => tag.name));
  const tagSuggestions = Array.from(new Set(tags.map((tag) => tag.name))).sort((left, right) => left.localeCompare(right, "zh-CN"));
  const toggleNetwork = (code: string) => setSelectedNetworks((current) => current.includes(code) ? current.filter((item) => item !== code) : [...current, code]);
  return <FormShell label={value ? "保存标的" : "创建标的"} submit={async () => {
    const savedInstrument = await api<Instrument>(value ? `/instruments/${value.id}` : "/instruments", { method: value ? "PUT" : "POST", body: JSON.stringify({ symbol, name, asset_type: type, currency, exchange: exchange || null, network: selectedNetworks.length ? selectedNetworks.join(",") : null, contract_address: contract || null, precision: Number(precision) }) });
    await api<InstrumentTag[]>(`/instruments/${savedInstrument.id}/tags`, { method: "PUT", body: JSON.stringify({ tags: selectedTags }) });
    await saved(value ? "标的与类别标签已更新" : "标的与类别标签已创建");
  }}><div className="form-grid"><label>代码<input value={symbol} onChange={(e) => setSymbol(e.target.value.toUpperCase())} required /></label><label>资产类别<select value={type} onChange={(e) => setType(e.target.value)}>{Object.entries(assetLabels).map(([id, label]) => <option key={id} value={id}>{label}</option>)}</select></label></div><label>名称<input value={name} onChange={(e) => setName(e.target.value)} required /></label><InstrumentTagEditor values={selectedTags} suggestions={tagSuggestions} onChange={setSelectedTags} /><div className="form-grid"><label>计价币种<MajorCurrencySelect value={currency} onChange={setCurrency} /></label><label>市场<input value={exchange} onChange={(e) => setExchange(e.target.value)} placeholder="SSE / NASDAQ" /></label></div>{["crypto", "stablecoin"].includes(type) && <><NetworkMultiChoice networks={networks} values={selectedNetworks} onToggle={toggleNetwork} /><label>精度<input type="number" min="0" max="30" value={precision} onChange={(e) => setPrecision(e.target.value)} /></label><label>合约地址<input value={contract} onChange={(e) => setContract(e.target.value)} /></label></>}</FormShell>;
}

function NetworkForm({ value, saved }: { value?: BlockchainNetwork; saved: (message: string) => Promise<void> }) { const [code, setCode] = useState(value?.code ?? ""); const [name, setName] = useState(value?.name ?? ""); return <FormShell label={value ? "保存网络" : "创建网络"} submit={async () => { await api(value ? `/blockchain-networks/${value.id}` : "/blockchain-networks", { method: value ? "PUT" : "POST", body: JSON.stringify({ code, name }) }); await saved(value ? "区块链网络已更新" : "区块链网络已创建"); }}><label>网络名称<input value={name} onChange={(event) => setName(event.target.value)} placeholder="例如 Ethereum" required /></label><label>网络代码<input value={code} onChange={(event) => setCode(event.target.value.toLowerCase().replace(/\s+/g, "-"))} placeholder="例如 ethereum" required /></label><p className="form-help">网络代码用于标的关联，只能包含字母、数字、连字符或下划线；网络被标的使用后不能修改代码。</p></FormShell>; }

function TransactionForm({ value, data, saved }: { value?: Transaction; data: Snapshot; saved: (message: string) => Promise<void> }) {
  const [advanced, setAdvanced] = useState(Boolean(value));
  if (!advanced) return <StandardTransactionForm data={data} saved={saved} onAdvanced={() => setAdvanced(true)} />;
  return <div className="transaction-entry">{!value && <div className="transaction-mode-banner"><span><strong>高级分录模式</strong><small>适合跨币种、公司行动和其他非标准业务。</small></span><button type="button" onClick={() => setAdvanced(false)}>返回标准模式</button></div>}<AdvancedTransactionForm value={value} data={data} saved={saved} /></div>;
}

function StandardTransactionForm({ data, saved, onAdvanced }: { data: Snapshot; saved: (message: string) => Promise<void>; onAdvanced: () => void }) {
  const activeAccounts = data.accounts.filter((item) => item.is_active);
  const activeInstruments = data.instruments.filter((item) => item.is_active);
  const cashInstruments = activeInstruments.filter((item) => ["cash", "stablecoin"].includes(item.asset_type)).sort((left, right) => Number(right.asset_type === "cash") - Number(left.asset_type === "cash"));
  const assetInstruments = activeInstruments.filter((item) => item.asset_type !== "cash");
  const settlementCashCandidatesFor = (instrument: Instrument | undefined) => {
    if (!instrument) return [];
    const digitalAsset = ["crypto", "stablecoin"].includes(instrument.asset_type);
    return cashInstruments.filter((item) => item.id !== instrument.id && item.currency === instrument.currency && (digitalAsset || item.asset_type === "cash"));
  };
  const recent = data.transactions[0];
  const recentAccount = recent?.legs.find((leg) => activeAccounts.some((item) => item.id === leg.account_id))?.account_id;
  const recentAsset = recent?.legs.find((leg) => leg.leg_type === "asset" && assetInstruments.some((item) => item.id === leg.instrument_id))?.instrument_id;
  const defaultAccountId = recentAccount ?? activeAccounts[0]?.id ?? "";
  const defaultAssetId = recentAsset ?? assetInstruments[0]?.id ?? "";
  const defaultAsset = assetInstruments.find((item) => item.id === defaultAssetId);
  const recentCash = recent?.legs.find((leg) => cashInstruments.some((item) => item.id === leg.instrument_id))?.instrument_id;
  const defaultSettlementCash = settlementCashCandidatesFor(defaultAsset);
  const defaultCashId = defaultSettlementCash.find((item) => item.id === recentCash)?.id ?? defaultSettlementCash[0]?.id ?? cashInstruments.find((item) => item.asset_type === "cash" && item.currency === activeAccounts.find((account) => account.id === defaultAccountId)?.base_currency)?.id ?? cashInstruments.find((item) => item.asset_type === "cash")?.id ?? cashInstruments[0]?.id ?? "";
  const [type, setType] = useState<StandardTransactionType>("buy");
  const [accountId, setAccountId] = useState(defaultAccountId);
  const [toAccountId, setToAccountId] = useState(activeAccounts.find((item) => item.id !== defaultAccountId)?.id ?? defaultAccountId);
  const [instrumentId, setInstrumentId] = useState(defaultAssetId);
  const [cashInstrumentId, setCashInstrumentId] = useState(defaultCashId);
  const [quantity, setQuantity] = useState("1");
  const [price, setPrice] = useState("100");
  const [amount, setAmount] = useState("1000");
  const [tradeAt, setTradeAt] = useState(localDateTime());
  const [memo, setMemo] = useState("");
  const [externalId, setExternalId] = useState("");
  const [instrumentQuery, setInstrumentQuery] = useState("");
  const [instrumentTypeFilter, setInstrumentTypeFilter] = useState("all");
  const [instrumentTagFilter, setInstrumentTagFilter] = useState("all");

  const selectedAccount = activeAccounts.find((item) => item.id === accountId);
  const selectedInstrument = assetInstruments.find((item) => item.id === instrumentId);
  const buyOrSell = type === "buy" || type === "sell";
  const reward = type === "staking_reward" || type === "airdrop";
  const cashEvent = !buyOrSell && !reward && type !== "transfer";
  const eligibleCash = buyOrSell ? settlementCashCandidatesFor(selectedInstrument) : cashInstruments;
  const selectedCash = cashInstruments.find((item) => item.id === cashInstrumentId);
  const grossAmount = multiplyDecimalStrings(quantity, price);
  const tagNamesFor = (id: string) => data.instrumentTags.filter((tag) => tag.instrument_id === id).map((tag) => tag.name);
  const availableInstrumentTypes = Array.from(new Set(assetInstruments.map((instrument) => instrument.asset_type)));
  const availableInstrumentTags = Array.from(new Set(data.instrumentTags.filter((tag) => assetInstruments.some((instrument) => instrument.id === tag.instrument_id)).map((tag) => tag.name))).sort((left, right) => left.localeCompare(right, "zh-CN"));
  const filterAssetInstruments = (query: string, assetType: string, tagFilter: string) => assetInstruments.filter((instrument) => {
    const tags = tagNamesFor(instrument.id);
    const matchesQuery = !query.trim() || `${instrument.symbol} ${instrument.name} ${tags.join(" ")}`.toLowerCase().includes(query.trim().toLowerCase());
    const matchesType = assetType === "all" || instrument.asset_type === assetType;
    const matchesTag = tagFilter === "all" || tags.includes(tagFilter);
    return matchesQuery && matchesType && matchesTag;
  });
  const filteredAssetInstruments = filterAssetInstruments(instrumentQuery, instrumentTypeFilter, instrumentTagFilter);
  const typeHelp: Record<StandardTransactionType, string> = {
    buy: "填写成交数量和单价，系统自动增加资产并扣减结算现金。", sell: "填写卖出数量和单价，系统自动减少资产并增加结算现金。",
    deposit: "记录组合外部入金，金额按正数录入。", withdrawal: "记录组合外部出金，系统自动处理为现金减少。", transfer: "只填写一次金额，自动生成转出和转入两条分录。",
    dividend: "记录现金分红并计入投资收益。", interest: "记录利息收入并计入投资收益。", return_of_capital: "记录资本返还形成的现金流入。",
    fee: "记录手续费或管理费，系统自动处理为现金减少。", tax: "记录交易税费，系统自动处理为现金减少。", staking_reward: "记录收到的虚拟货币质押奖励。", airdrop: "记录收到的空投资产。",
  };

  const chooseCashFor = (currency: string | undefined, account: Account | undefined, instrument?: Instrument) => {
    const candidates = instrument ? settlementCashCandidatesFor(instrument) : cashInstruments;
    const candidate = candidates.find((item) => item.currency === currency) ?? (!instrument ? candidates.find((item) => item.currency === account?.base_currency) ?? candidates[0] : undefined);
    setCashInstrumentId(candidate?.id ?? "");
  };
  const changeType = (next: StandardTransactionType) => {
    setType(next);
    if (next === "buy" || next === "sell") chooseCashFor(selectedInstrument?.currency, selectedAccount, selectedInstrument);
    else chooseCashFor(selectedAccount?.base_currency, selectedAccount);
  };
  const changeAccount = (next: string) => {
    setAccountId(next);
    const account = activeAccounts.find((item) => item.id === next);
    chooseCashFor(buyOrSell ? selectedInstrument?.currency : account?.base_currency, account, buyOrSell ? selectedInstrument : undefined);
    if (toAccountId === next) setToAccountId(activeAccounts.find((item) => item.id !== next)?.id ?? next);
  };
  const changeInstrument = (next: string) => {
    setInstrumentId(next);
    const instrument = assetInstruments.find((item) => item.id === next);
    if (buyOrSell) chooseCashFor(instrument?.currency, selectedAccount, instrument);
  };
  const updateInstrumentFilters = (next: { query?: string; assetType?: string; tag?: string }) => {
    const query = next.query ?? instrumentQuery;
    const assetType = next.assetType ?? instrumentTypeFilter;
    const tag = next.tag ?? instrumentTagFilter;
    setInstrumentQuery(query);
    setInstrumentTypeFilter(assetType);
    setInstrumentTagFilter(tag);
    const matches = filterAssetInstruments(query, assetType, tag);
    if (!matches.some((instrument) => instrument.id === instrumentId)) changeInstrument(matches[0]?.id ?? "");
  };
  const requirePositive = (raw: string, label: string) => {
    if (!/^\d+(?:\.\d+)?$/.test(raw.trim()) || number(raw) <= 0) throw new Error(`${label}必须是大于 0 的数字`);
    return raw.trim().replace(/^0+(?=\d)/, "");
  };
  const buildPayload = (): TransactionPayload => {
    if (!accountId) throw new Error("请先创建并启用一个账户");
    if (!cashInstrumentId && !reward) throw new Error("缺少可用的现金或稳定币标的，请先在数据页面新增");
    if ((buyOrSell || reward) && !instrumentId) throw new Error("请选择投资标的");
    const normalizedQuantity = (buyOrSell || reward) ? requirePositive(quantity, "数量") : "";
    const normalizedAmount = (cashEvent || type === "transfer") ? requirePositive(amount, "金额") : "";
    const normalizedPrice = buyOrSell ? requirePositive(price, "成交价") : "";
    const gross = buyOrSell ? multiplyDecimalStrings(normalizedQuantity, normalizedPrice) : "";
    if (buyOrSell && !gross) throw new Error("无法计算成交金额，请检查数量和单价");
    if (buyOrSell && selectedCash?.currency !== selectedInstrument?.currency) throw new Error("结算现金币种必须与标的计价币种一致；跨币种交易请使用高级分录模式");
    if (type === "transfer" && accountId === toAccountId) throw new Error("转出账户和转入账户不能相同");
    const legs: TransactionPayload["legs"] = [];
    if (buyOrSell) {
      const assetDirection = type === "buy" ? normalizedQuantity : `-${normalizedQuantity}`;
      const cashDirection = type === "buy" ? `-${gross}` : gross;
      legs.push({ account_id: accountId, instrument_id: instrumentId, leg_type: "asset", quantity: assetDirection, unit_price: normalizedPrice, price_currency: selectedInstrument?.currency ?? null, memo: null });
      legs.push({ account_id: accountId, instrument_id: cashInstrumentId, leg_type: "cash", quantity: cashDirection, unit_price: null, price_currency: null, memo: null });
    } else if (type === "transfer") {
      legs.push({ account_id: accountId, instrument_id: cashInstrumentId, leg_type: "cash", quantity: `-${normalizedAmount}`, unit_price: null, price_currency: null, memo: null });
      legs.push({ account_id: toAccountId, instrument_id: cashInstrumentId, leg_type: "cash", quantity: normalizedAmount, unit_price: null, price_currency: null, memo: null });
    } else if (reward) {
      legs.push({ account_id: accountId, instrument_id: instrumentId, leg_type: "asset", quantity: normalizedQuantity, unit_price: null, price_currency: null, memo: null });
    } else {
      const negative = type === "withdrawal" || type === "fee" || type === "tax";
      legs.push({ account_id: accountId, instrument_id: cashInstrumentId, leg_type: "cash", quantity: negative ? `-${normalizedAmount}` : normalizedAmount, unit_price: null, price_currency: null, memo: null });
    }
    return { transaction_type: type, trade_at: new Date(tradeAt).toISOString(), source: "web_standard", external_id: externalId.trim() || null, memo: memo.trim() || null, legs };
  };

  return <FormShell label="确认并写入账本" submit={async () => {
    const payload = buildPayload();
    const duplicate = await api<DuplicateCheck>("/transactions/duplicate-check", { method: "POST", body: JSON.stringify(payload) });
    if (duplicate.duplicate) {
      const first = duplicate.matches[0];
      const description = first ? `${dateText(first.trade_at)}${first.memo ? ` · ${first.memo}` : ""}` : "相近时间";
      if (!window.confirm(`检测到 ${duplicate.matches.length} 条分录内容相同的流水，最近一条为：${description}。\n\n仍要继续写入吗？`)) return;
    }
    await api("/transactions", { method: "POST", body: JSON.stringify(payload) });
    await saved("标准流水已生成并写入账本");
  }}>
    <div className="transaction-mode-banner standard"><span><strong>标准录入模式</strong><small>正负方向、现金金额、币种和复式分录均由系统生成。</small></span><button type="button" onClick={onAdvanced}>高级分录</button></div>
    <label>业务类型<select value={type} onChange={(event) => changeType(event.target.value as StandardTransactionType)}><optgroup label="证券交易"><option value="buy">买入</option><option value="sell">卖出</option></optgroup><optgroup label="资金变动"><option value="deposit">入金</option><option value="withdrawal">出金</option><option value="transfer">账户间转账</option></optgroup><optgroup label="投资收益"><option value="dividend">分红</option><option value="interest">利息</option><option value="return_of_capital">资本返还</option><option value="staking_reward">质押奖励</option><option value="airdrop">空投</option></optgroup><optgroup label="费用支出"><option value="fee">手续费</option><option value="tax">税费</option></optgroup></select><small className="field-help">{typeHelp[type]}</small></label>
    <div className="form-grid"><label>{type === "transfer" ? "转出账户" : "账户"}<select value={accountId} onChange={(event) => changeAccount(event.target.value)}>{activeAccounts.map((account) => <option key={account.id} value={account.id}>{account.name} · {account.base_currency}</option>)}</select></label>{type === "transfer" ? <label>转入账户<select value={toAccountId} onChange={(event) => setToAccountId(event.target.value)}>{activeAccounts.map((account) => <option key={account.id} value={account.id}>{account.name} · {account.base_currency}</option>)}</select></label> : <label>发生时间<input type="datetime-local" value={tradeAt} onChange={(event) => setTradeAt(event.target.value)} required /></label>}</div>
    {type === "transfer" && <label>发生时间<input type="datetime-local" value={tradeAt} onChange={(event) => setTradeAt(event.target.value)} required /></label>}
    {(buyOrSell || reward) && <div className="instrument-picker"><div className="instrument-picker-head"><strong>选择投资标的</strong><span>显示 {filteredAssetInstruments.length} / {assetInstruments.length} 项</span></div><div className="instrument-picker-filters"><label>关键词<input value={instrumentQuery} onChange={(event) => updateInstrumentFilters({ query: event.target.value })} placeholder="名称、代码或标签" /></label><label>资产类别<select value={instrumentTypeFilter} onChange={(event) => updateInstrumentFilters({ assetType: event.target.value })}><option value="all">全部类别</option>{availableInstrumentTypes.map((assetType) => <option key={assetType} value={assetType}>{assetLabels[assetType] ?? assetType}</option>)}</select></label><label>类别标签<select value={instrumentTagFilter} onChange={(event) => updateInstrumentFilters({ tag: event.target.value })}><option value="all">全部标签</option>{availableInstrumentTags.map((tag) => <option key={tag} value={tag}>{tag}</option>)}</select></label></div><label>投资标的<select value={instrumentId} disabled={filteredAssetInstruments.length === 0} onChange={(event) => changeInstrument(event.target.value)}>{!filteredAssetInstruments.length && <option value="">没有匹配的投资标的</option>}{filteredAssetInstruments.map((instrument) => { const tags = tagNamesFor(instrument.id); return <option key={instrument.id} value={instrument.id}>{instrument.symbol} · {instrument.name} · {assetLabels[instrument.asset_type] ?? instrument.asset_type}{tags.length ? ` · ${tags.join(" / ")}` : ""}</option>; })}</select></label>{filteredAssetInstruments.length === 0 && <small className="field-warning">当前筛选没有匹配项，投资标的已清空；请修改筛选条件。</small>}</div>}
    {(buyOrSell || cashEvent || type === "transfer") && <label>{buyOrSell ? `结算现金（${selectedInstrument?.currency ?? "—"}）` : "现金或稳定币"}<select value={cashInstrumentId} onChange={(event) => setCashInstrumentId(event.target.value)}>{eligibleCash.map((instrument) => <option key={instrument.id} value={instrument.id}>{instrument.symbol} · {instrument.name} · {instrument.currency}</option>)}</select>{buyOrSell && eligibleCash.length === 0 && <small className="field-warning">没有与标的计价币种匹配的现金标的，请先新增或改用高级分录。</small>}</label>}
    {buyOrSell && <div className="form-grid"><label>成交数量<input inputMode="decimal" value={quantity} onChange={(event) => setQuantity(event.target.value)} required /></label><label>成交单价（{selectedInstrument?.currency ?? "—"}）<input inputMode="decimal" value={price} onChange={(event) => setPrice(event.target.value)} required /></label></div>}
    {reward && <label>收到数量<input inputMode="decimal" value={quantity} onChange={(event) => setQuantity(event.target.value)} required /></label>}
    {(cashEvent || type === "transfer") && <label>金额（{selectedCash?.currency ?? "—"}）<input inputMode="decimal" value={amount} onChange={(event) => setAmount(event.target.value)} required /></label>}
    <div className="generated-entry-preview"><div><span>系统将自动生成</span><strong>{buyOrSell ? "资产 + 现金 2 条分录" : type === "transfer" ? "转出 + 转入 2 条分录" : "1 条标准分录"}</strong></div><b>{buyOrSell ? `${grossAmount || "—"} ${selectedInstrument?.currency ?? ""}` : type === "transfer" || cashEvent ? `${amount || "—"} ${selectedCash?.currency ?? ""}` : `${quantity || "—"} ${selectedInstrument?.symbol ?? ""}`}</b></div>
    <details className="transaction-optional"><summary>备注与去重信息（可选）</summary><div><label>备注<input value={memo} onChange={(event) => setMemo(event.target.value)} placeholder="例如：定投、工资入金、季度分红" /></label><label>外部流水号<input value={externalId} onChange={(event) => setExternalId(event.target.value)} placeholder="券商或银行流水号，可用于精确去重" /></label></div></details>
  </FormShell>;
}

function AdvancedTransactionForm({ value, data, saved }: { value?: Transaction; data: Snapshot; saved: (message: string) => Promise<void> }) { const defaultAsset = data.instruments.find((item) => !["cash", "stablecoin"].includes(item.asset_type))?.id ?? data.instruments[0]?.id ?? ""; const defaultCash = data.instruments.find((item) => item.asset_type === "cash")?.id ?? data.instruments[0]?.id ?? ""; const defaultAccount = data.accounts.find((item) => item.is_active)?.id ?? data.accounts[0]?.id ?? ""; const [type, setType] = useState(value?.transaction_type ?? "buy"); const [tradeAt, setTradeAt] = useState(localDateTime(value?.trade_at)); const [memo, setMemo] = useState(value?.memo ?? ""); const [externalId, setExternalId] = useState(value?.external_id ?? ""); const [legs, setLegs] = useState<Leg[]>(value?.legs ?? [{ account_id: defaultAccount, instrument_id: defaultAsset, leg_type: "asset", quantity: "1", unit_price: "100", price_currency: data.instruments.find((item) => item.id === defaultAsset)?.currency ?? "CNY" }, { account_id: defaultAccount, instrument_id: defaultCash, leg_type: "cash", quantity: "-100", unit_price: null, price_currency: null }]); function preset(next: string) { setType(next); const cash = { account_id: defaultAccount, instrument_id: defaultCash, leg_type: "cash", quantity: ["withdrawal", "fee", "tax"].includes(next) ? "-100" : "100", unit_price: null, price_currency: null }; if (["buy", "sell"].includes(next)) setLegs([{ account_id: defaultAccount, instrument_id: defaultAsset, leg_type: "asset", quantity: next === "buy" ? "1" : "-1", unit_price: "100", price_currency: data.instruments.find((item) => item.id === defaultAsset)?.currency ?? "CNY" }, { ...cash, quantity: next === "buy" ? "-100" : "100" }]); else if (next === "transfer") setLegs([{ ...cash, quantity: "-100" }, { ...cash, quantity: "100" }]); else setLegs([cash]); } function update(index: number, patch: Partial<Leg>) { setLegs((current) => current.map((leg, legIndex) => legIndex === index ? { ...leg, ...patch } : leg)); } return <FormShell label={value ? "保存更正" : "写入账本"} submit={async () => { const payload = { transaction_type: type, trade_at: new Date(tradeAt).toISOString(), source: value ? "web_correction" : "web", external_id: externalId || null, memo: memo || null, legs: legs.map((leg) => ({ ...leg, unit_price: leg.unit_price || null, price_currency: leg.unit_price ? (leg.price_currency || data.instruments.find((item) => item.id === leg.instrument_id)?.currency || "CNY") : null, memo: null })) }; await api(value ? `/transactions/${value.id}` : "/transactions", { method: value ? "PUT" : "POST", body: JSON.stringify(payload) }); await saved(value ? "原流水已冲销，正确流水已重记" : "流水已写入账本"); }}><div className="correction-note">{value ? "更正将保留原流水，并自动生成冲销记录。" : "每笔业务由一条或多条资产、现金、费用分录组成。"}</div><label>事件类型<select value={type} onChange={(e) => preset(e.target.value)}>{Object.entries(transactionLabels).map(([id, label]) => <option key={id} value={id}>{label}</option>)}</select></label><div className="form-grid"><label>交易时间<input type="datetime-local" value={tradeAt} onChange={(e) => setTradeAt(e.target.value)} required /></label><label>外部流水号<input value={externalId} onChange={(e) => setExternalId(e.target.value)} placeholder="可选，用于去重" /></label></div><label>备注<input value={memo} onChange={(e) => setMemo(e.target.value)} /></label><div className="legs-head"><strong>分录</strong><button type="button" onClick={() => setLegs((current) => [...current, { account_id: defaultAccount, instrument_id: defaultCash, leg_type: "cash", quantity: "0", unit_price: null, price_currency: null }])}>＋ 添加分录</button></div>{legs.map((leg, index) => <div className="leg-card" key={index}><div className="form-grid"><label>账户<select value={leg.account_id} onChange={(e) => update(index, { account_id: e.target.value })}>{data.accounts.filter((item) => item.is_active).map((item) => <option key={item.id} value={item.id}>{item.name}</option>)}</select></label><label>标的<select value={leg.instrument_id} onChange={(e) => update(index, { instrument_id: e.target.value })}>{data.instruments.filter((item) => item.is_active).map((item) => <option key={item.id} value={item.id}>{item.symbol} · {item.name}</option>)}</select></label></div><div className="form-grid three"><label>分录类型<select value={leg.leg_type} onChange={(e) => update(index, { leg_type: e.target.value })}>{["asset", "cash", "fee", "tax", "income", "other"].map((item) => <option key={item}>{item}</option>)}</select></label><label>数量（含正负号）<input inputMode="decimal" value={leg.quantity} onChange={(e) => update(index, { quantity: e.target.value })} required /></label><label>单价<input inputMode="decimal" value={leg.unit_price ?? ""} onChange={(e) => update(index, { unit_price: e.target.value || null })} placeholder="可选" /></label></div>{legs.length > 1 && <button type="button" className="remove-leg" onClick={() => setLegs((current) => current.filter((_, legIndex) => legIndex !== index))}>移除此分录</button>}</div>)}</FormShell>; }

function PriceForm({ value, instruments, saved }: { value?: Price; instruments: Instrument[]; saved: (message: string) => Promise<void> }) { const [instrument, setInstrument] = useState(value?.instrument_id ?? instruments[0]?.id ?? ""); const selected = instruments.find((item) => item.id === instrument); const [price, setPrice] = useState(value?.price ?? ""); const [currency, setCurrency] = useState(normalizeMajorCurrency(value?.currency ?? selected?.currency)); const [time, setTime] = useState(localDateTime(value?.price_at)); const [source, setSource] = useState(value?.source ?? "manual"); return <FormShell label="保存价格" submit={async () => { await api("/prices", { method: "POST", body: JSON.stringify({ instrument_id: instrument, price_at: new Date(time).toISOString(), price, currency, source, is_manual_override: true }) }); await saved("价格已更新"); }}><label>标的<select value={instrument} onChange={(e) => { setInstrument(e.target.value); setCurrency(normalizeMajorCurrency(instruments.find((item) => item.id === e.target.value)?.currency)); }}>{instruments.map((item) => <option value={item.id} key={item.id}>{item.symbol} · {item.name}</option>)}</select></label><div className="form-grid"><label>价格<input inputMode="decimal" value={price} onChange={(e) => setPrice(e.target.value)} required /></label><label>币种<MajorCurrencySelect value={currency} onChange={setCurrency} /></label></div><div className="form-grid"><label>价格时间<input type="datetime-local" value={time} onChange={(e) => setTime(e.target.value)} required /></label><label>来源<input value={source} onChange={(e) => setSource(e.target.value)} required /></label></div></FormShell>; }
function FxForm({ value, saved }: { value?: Fx; saved: (message: string) => Promise<void> }) { const [base, setBase] = useState(normalizeMajorCurrency(value?.base_currency, "USD")); const [quote, setQuote] = useState(normalizeMajorCurrency(value?.quote_currency)); const [rate, setRate] = useState(value?.rate ?? ""); const [time, setTime] = useState(localDateTime(value?.rate_at)); const [source, setSource] = useState(value?.source ?? "manual"); return <FormShell label="保存汇率" submit={async () => { await api("/fx-rates", { method: "POST", body: JSON.stringify({ base_currency: base, quote_currency: quote, rate_at: new Date(time).toISOString(), rate, source }) }); await saved("汇率已更新"); }}><div className="form-grid"><label>基础币种<MajorCurrencySelect value={base} onChange={setBase} /></label><label>报价币种<MajorCurrencySelect value={quote} onChange={setQuote} /></label></div><label>汇率<input inputMode="decimal" value={rate} onChange={(e) => setRate(e.target.value)} required /></label><div className="form-grid"><label>汇率时间<input type="datetime-local" value={time} onChange={(e) => setTime(e.target.value)} required /></label><label>来源<input value={source} onChange={(e) => setSource(e.target.value)} required /></label></div></FormShell>; }
function TargetForm({ value, saved }: { value?: Target; saved: (message: string) => Promise<void> }) { const [dimension, setDimension] = useState(value?.dimension ?? "asset_type"); const [name, setName] = useState(value?.value ?? "etf"); const [target, setTarget] = useState(value?.target_weight ?? "0.60"); const [min, setMin] = useState(value?.min_weight ?? "0.50"); const [max, setMax] = useState(value?.max_weight ?? "0.70"); return <FormShell label="保存配置目标" submit={async () => { await api(value ? `/targets/${value.id}` : "/targets", { method: value ? "PUT" : "POST", body: JSON.stringify({ dimension, value: name, target_weight: target, min_weight: min, max_weight: max }) }); await saved("配置目标已保存"); }}><div className="form-grid"><label>维度<select value={dimension} onChange={(e) => setDimension(e.target.value)}><option value="asset_type">资产类型</option><option value="currency">币种</option><option value="account">账户</option></select></label><label>分类值<input value={name} onChange={(e) => setName(e.target.value)} required /></label></div><div className="form-grid three"><label>目标权重<input value={target} onChange={(e) => setTarget(e.target.value)} /></label><label>最小权重<input value={min} onChange={(e) => setMin(e.target.value)} /></label><label>最大权重<input value={max} onChange={(e) => setMax(e.target.value)} /></label></div></FormShell>; }
function DecisionForm({ value, instruments, saved }: { value?: Decision; instruments: Instrument[]; saved: (message: string) => Promise<void> }) { const [instrument, setInstrument] = useState(value?.instrument_id ?? ""); const [action, setAction] = useState(value?.action ?? ""); const [time, setTime] = useState(localDateTime(value?.decided_at)); const [rationale, setRationale] = useState(value?.rationale ?? ""); const [confidence, setConfidence] = useState(value?.confidence ?? 50); const [risks, setRisks] = useState(value?.risks ?? ""); const [invalidation, setInvalidation] = useState(value?.invalidation ?? ""); const [reviewAt, setReviewAt] = useState(value?.review_at ? localDateTime(value.review_at) : ""); const [outcome, setOutcome] = useState(value?.outcome ?? ""); return <FormShell label="保存决策" submit={async () => { await api(value ? `/decisions/${value.id}` : "/decisions", { method: value ? "PUT" : "POST", body: JSON.stringify({ instrument_id: instrument || null, action, decided_at: new Date(time).toISOString(), rationale, confidence, risks, invalidation, review_at: reviewAt ? new Date(reviewAt).toISOString() : null, outcome, process_score: value?.process_score ?? null, result_score: value?.result_score ?? null }) }); await saved("决策记录已保存"); }}><label>关联标的<select value={instrument} onChange={(e) => setInstrument(e.target.value)}><option value="">不关联具体标的</option>{instruments.map((item) => <option key={item.id} value={item.id}>{item.symbol} · {item.name}</option>)}</select></label><label>决策动作<input value={action} onChange={(e) => setAction(e.target.value)} required /></label><label>决策理由<textarea value={rationale} onChange={(e) => setRationale(e.target.value)} required /></label><div className="form-grid"><label>决策时间<input type="datetime-local" value={time} onChange={(e) => setTime(e.target.value)} /></label><label>信心 {confidence}%<input type="range" min="0" max="100" value={confidence} onChange={(e) => setConfidence(Number(e.target.value))} /></label></div><label>主要风险<textarea value={risks} onChange={(e) => setRisks(e.target.value)} /></label><label>证伪条件<textarea value={invalidation} onChange={(e) => setInvalidation(e.target.value)} /></label><label>复盘时间<input type="datetime-local" value={reviewAt} onChange={(e) => setReviewAt(e.target.value)} /></label><label>实际结果<textarea value={outcome} onChange={(e) => setOutcome(e.target.value)} /></label></FormShell>; }
function ReviewForm({ value, saved }: { value?: Review; saved: (message: string) => Promise<void> }) { const [type, setType] = useState(value?.period_type ?? "monthly"); const today = new Date().toISOString().slice(0, 10); const [start, setStart] = useState(value?.period_start ?? today.slice(0, 8) + "01"); const [end, setEnd] = useState(value?.period_end ?? today); const [summary, setSummary] = useState(value?.summary ?? ""); const [actions, setActions] = useState(value?.actions ?? ""); const [completed, setCompleted] = useState(Boolean(value?.completed_at)); return <FormShell label="保存复盘" submit={async () => { await api(value ? `/reviews/${value.id}` : "/reviews", { method: value ? "PUT" : "POST", body: JSON.stringify({ period_type: type, period_start: start, period_end: end, summary, actions, completed_at: completed ? new Date().toISOString() : null }) }); await saved("复盘记录已保存"); }}><label>复盘类型<select value={type} onChange={(e) => setType(e.target.value)}><option value="weekly">每周</option><option value="monthly">每月</option><option value="quarterly">每季度</option><option value="annual">年度</option></select></label><div className="form-grid"><label>开始日期<input type="date" value={start} onChange={(e) => setStart(e.target.value)} /></label><label>结束日期<input type="date" value={end} onChange={(e) => setEnd(e.target.value)} /></label></div><label>复盘总结<textarea value={summary} onChange={(e) => setSummary(e.target.value)} required /></label><label>下一步行动<textarea value={actions} onChange={(e) => setActions(e.target.value)} /></label><label className="check"><input type="checkbox" checked={completed} onChange={(e) => setCompleted(e.target.checked)} />标记为已完成</label></FormShell>; }
function SourceForm({ value, saved }: { value?: Source; saved: (message: string) => Promise<void> }) { const [name, setName] = useState(value?.name ?? "CoinGecko 公开行情"); const [type, setType] = useState(value?.source_type ?? "market_data"); const [priority, setPriority] = useState(value?.priority ?? 100); const [enabled, setEnabled] = useState(value?.is_enabled ?? true); const [config, setConfig] = useState(JSON.stringify(value?.config ?? { provider: "coingecko_simple", instrument_id: "请替换为标的UUID", coin_id: "bitcoin", vs_currency: "cny" }, null, 2)); return <FormShell label="保存数据源" submit={async () => { let parsed; try { parsed = JSON.parse(config); } catch { throw new Error("配置必须是有效 JSON"); } await api(value ? `/data-sources/${value.id}` : "/data-sources", { method: value ? "PUT" : "POST", body: JSON.stringify({ name, source_type: type, priority, credentials_ref: null, config: parsed, is_enabled: enabled }) }); await saved("API 数据源已保存"); }}><label>名称<input value={name} onChange={(e) => setName(e.target.value)} /></label><div className="form-grid"><label>类型<select value={type} onChange={(e) => setType(e.target.value)}><option value="market_data">市场行情</option><option value="fx">汇率</option><option value="benchmark">基准</option><option value="broker">券商只读</option><option value="crypto_exchange">交易所只读</option><option value="blockchain">链上只读</option></select></label><label>优先级<input type="number" value={priority} onChange={(e) => setPriority(Number(e.target.value))} /></label></div><label>公开 API 配置<textarea className="code-area" value={config} onChange={(e) => setConfig(e.target.value)} /></label><p className="form-help">支持 tencent_quote、coingecko_simple、frankfurter 和 generic_json。通用地址仅允许公网 HTTPS，私有网络地址会被拒绝。</p><label className="check"><input type="checkbox" checked={enabled} onChange={(e) => setEnabled(e.target.checked)} />启用数据源</label></FormShell>; }
function JobForm({ value, sources, saved }: { value?: Job; sources: Source[]; saved: (message: string) => Promise<void> }) { const [source, setSource] = useState(value?.data_source_id ?? sources[0]?.id ?? ""); const [name, setName] = useState(value?.name ?? "行情定时更新"); const [type, setType] = useState(value?.data_type ?? "prices"); const [minutes, setMinutes] = useState(Math.max(1, Math.round((value?.interval_seconds ?? 900) / 60))); const [timezone, setTimezone] = useState(timezoneOptions.some((item) => item.value === value?.timezone) ? value!.timezone : "Asia/Shanghai"); const [enabled, setEnabled] = useState(value?.is_enabled ?? true); return <FormShell label="保存同步任务" submit={async () => { await api(value ? `/sync-jobs/${value.id}` : "/sync-jobs", { method: value ? "PUT" : "POST", body: JSON.stringify({ data_source_id: source, name, data_type: type, interval_seconds: minutes * 60, timezone, retry_policy: { max_retries: 3, backoff: "exponential" }, is_enabled: enabled }) }); await saved("同步任务已保存"); }}><label>数据源<select value={source} onChange={(e) => setSource(e.target.value)}>{sources.map((item) => <option key={item.id} value={item.id}>{item.name}</option>)}</select></label><label>任务名称<input value={name} onChange={(e) => setName(e.target.value)} /></label><div className="form-grid"><label>数据类型<select value={type} onChange={(e) => setType(e.target.value)}><option value="prices">价格</option><option value="fx_rates">汇率</option><option value="balances">余额（待确认区）</option><option value="transactions">流水（待确认区）</option></select></label><label>执行间隔（分钟）<input type="number" min="1" value={minutes} onChange={(e) => setMinutes(Number(e.target.value))} /></label></div><label>时区<select value={timezone} onChange={(e) => setTimezone(e.target.value)}>{timezoneOptions.map((item) => <option key={item.value} value={item.value}>{item.label} · {item.value}</option>)}</select></label><label className="check"><input type="checkbox" checked={enabled} onChange={(e) => setEnabled(e.target.checked)} />启用自动调度</label></FormShell>; }

function ApiManagerForm({ value, data, saved }: { value?: ApiManagerSeed | Collector; data: Snapshot; saved: (message: string) => Promise<void> }) {
  const collector = value && "id" in value ? value : undefined;
  const seed = collector ? undefined : value as ApiManagerSeed | undefined;
  const stored = collector?.config ?? {};
  const cryptoInstruments = data.instruments.filter((item) => item.is_active && ["crypto", "stablecoin"].includes(item.asset_type));
  const storedAssets = Array.isArray(stored.assets) ? stored.assets.filter((item): item is Record<string, unknown> => Boolean(item) && typeof item === "object") : [];
  const storedInstrumentId = typeof stored.instrument_id === "string" ? stored.instrument_id : undefined;
  const requestedInstrument = data.instruments.find((item) => item.id === (storedInstrumentId ?? seed?.instrument_id));
  const initialInstrument = requestedInstrument ?? (seed?.mode === "crypto" ? cryptoInstruments[0] : undefined) ?? data.instruments.find((item) => !["cash", "crypto", "stablecoin"].includes(item.asset_type)) ?? data.instruments.find((item) => item.asset_type !== "cash");
  const storedProvider = String(stored.provider ?? "");
  const initialMode: "price" | "crypto" | "fx" = collector?.data_type === "fx_rates" || seed?.mode === "fx" ? "fx" : seed?.mode === "crypto" || storedAssets.length > 0 || (storedProvider === "coingecko_simple" && requestedInstrument && ["crypto", "stablecoin"].includes(requestedInstrument.asset_type)) ? "crypto" : "price";
  const [mode, setMode] = useState<"price" | "crypto" | "fx">(initialMode);
  const [name, setName] = useState(collector?.name ?? "");
  const [instrumentId, setInstrumentId] = useState(initialInstrument?.id ?? "");
  const [provider, setProvider] = useState(String(stored.provider ?? (initialMode === "fx" ? "frankfurter" : initialMode === "crypto" ? "coingecko_simple" : stockMarket(initialInstrument) ? "tencent_quote" : "generic_json")));
  const [quoteSymbol, setQuoteSymbol] = useState(String(stored.quote_symbol ?? stockQuoteSymbol(initialInstrument)));
  const [url, setUrl] = useState(String(stored.url ?? ""));
  const [valuePath, setValuePath] = useState(String(stored.value_path ?? "price"));
  const [currency, setCurrency] = useState(normalizeMajorCurrency(String(stored.vs_currency ?? stored.currency ?? (initialMode === "crypto" ? "USD" : initialInstrument?.currency ?? data.settings.report_currency))));
  const storedCryptoIds = storedAssets.map((item) => String(item.instrument_id ?? "")).filter((id) => cryptoInstruments.some((instrument) => instrument.id === id));
  const fallbackCryptoId = requestedInstrument && ["crypto", "stablecoin"].includes(requestedInstrument.asset_type) ? requestedInstrument.id : cryptoInstruments[0]?.id;
  const [cryptoInstrumentIds, setCryptoInstrumentIds] = useState<string[]>(storedCryptoIds.length ? [...new Set(storedCryptoIds)] : fallbackCryptoId ? [fallbackCryptoId] : []);
  const [cryptoKeys, setCryptoKeys] = useState<Record<string, string>>(() => Object.fromEntries(cryptoInstruments.map((instrument) => {
    const savedAsset = storedAssets.find((item) => item.instrument_id === instrument.id);
    const savedKey = String(savedAsset?.coin_id ?? savedAsset?.lookup_key ?? "");
    return [instrument.id, savedKey || (storedProvider === "generic_json" ? instrument.symbol : defaultCoinGeckoId(instrument))];
  })));
  const initialBase = normalizeMajorCurrency(String(stored.base ?? seed?.base ?? "USD"), "USD");
  const requestedQuote = normalizeMajorCurrency(String(stored.quote ?? seed?.quote ?? data.settings.report_currency));
  const initialQuote = requestedQuote === initialBase ? majorCurrencies.find((item) => item.code !== initialBase)?.code ?? "CNY" : requestedQuote;
  const [base, setBase] = useState(initialBase);
  const [quote, setQuote] = useState(initialQuote);
  const storedResponseMode = String(stored.response_mode ?? "single");
  const initialFxResponseMode: FxResponseMode = ["single", "currency_paths", "currency_map", "currency_list"].includes(storedResponseMode) ? storedResponseMode as FxResponseMode : "single";
  const storedQuotes = Array.isArray(stored.quotes) ? stored.quotes.map((item) => normalizeMajorCurrency(String(item))).filter((item) => item !== base) : [];
  const [fxResponseMode, setFxResponseMode] = useState<FxResponseMode>(initialFxResponseMode);
  const [quotes, setQuotes] = useState<string[]>(storedQuotes.length ? [...new Set(storedQuotes)] : [quote].filter((item) => item !== base));
  const storedFxValuePaths = stored.value_paths && typeof stored.value_paths === "object" && !Array.isArray(stored.value_paths) ? stored.value_paths as Record<string, unknown> : {};
  const [fxValuePaths, setFxValuePaths] = useState<Record<string, string>>(() => Object.fromEntries(majorCurrencies.map((item) => [item.code, typeof storedFxValuePaths[item.code] === "string" ? String(storedFxValuePaths[item.code]) : `data.rates.${item.code}`])));
  const [ratesPath, setRatesPath] = useState(String(stored.rates_path ?? "rates"));
  const [currencyField, setCurrencyField] = useState(String(stored.currency_field ?? "currency"));
  const [rateField, setRateField] = useState(String(stored.rate_field ?? (initialFxResponseMode === "currency_list" ? "rate" : "")));
  const initialPriceResponseMode: PriceResponseMode = storedResponseMode === "asset_list" ? "asset_list" : "asset_map";
  const [priceResponseMode, setPriceResponseMode] = useState<PriceResponseMode>(initialPriceResponseMode);
  const [pricesPath, setPricesPath] = useState(String(stored.prices_path ?? (storedProvider === "coingecko_simple" ? "$" : "data")));
  const [symbolField, setSymbolField] = useState(String(stored.symbol_field ?? "symbol"));
  const [priceField, setPriceField] = useState(String(stored.price_field ?? (initialPriceResponseMode === "asset_list" ? "price" : "")));
  const storedAuthType = String(stored.auth_type ?? "none");
  const [authType, setAuthType] = useState(["none", "header", "query", "bearer"].includes(storedAuthType) ? storedAuthType : "none");
  const [apiKeyName, setApiKeyName] = useState(String(stored.api_key_name ?? (storedAuthType === "query" ? "apikey" : "X-API-Key")));
  const [apiKey, setApiKey] = useState("");
  const [clearApiKey, setClearApiKey] = useState(false);
  const [priority, setPriority] = useState(collector?.priority ?? 100);
  const [minutes, setMinutes] = useState(collector ? Math.max(1, Math.round(collector.interval_seconds / 60)) : initialMode === "fx" ? 1440 : initialMode === "crypto" ? 15 : stockMarket(initialInstrument) ? 5 : 60);
  const [enabled, setEnabled] = useState(collector?.is_enabled ?? true);
  const [runNow, setRunNow] = useState(!collector);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<CollectorTestResult | null>(null);
  const [testError, setTestError] = useState("");
  const selected = data.instruments.find((item) => item.id === instrumentId);
  const generic = provider === "generic_json";
  const multiFx = mode === "fx" && generic && fxResponseMode !== "single";
  const multiCrypto = mode === "crypto";

  const changeInstrument = (id: string) => {
    const next = data.instruments.find((item) => item.id === id);
    setInstrumentId(id);
    setCurrency(normalizeMajorCurrency(next?.currency ?? data.settings.report_currency));
    if (stockMarket(next)) { setProvider("tencent_quote"); setQuoteSymbol(stockQuoteSymbol(next)); setMinutes(5); }
    else setProvider("generic_json");
  };

  const toggleCryptoInstrument = (instrument: Instrument) => {
    setCryptoInstrumentIds((current) => current.includes(instrument.id) ? current.filter((id) => id !== instrument.id) : [...current, instrument.id]);
    setCryptoKeys((current) => ({ ...current, [instrument.id]: current[instrument.id] || (provider === "coingecko_simple" ? defaultCoinGeckoId(instrument) : instrument.symbol) }));
    setTestResult(null);
  };

  const changeCryptoKey = (instrumentId: string, value: string) => {
    setCryptoKeys((current) => ({ ...current, [instrumentId]: value }));
    setTestResult(null);
  };

  const toggleFxQuote = (code: string) => {
    if (code === base) return;
    setQuotes((current) => current.includes(code) ? current.filter((item) => item !== code) : [...current, code]);
    setTestResult(null);
  };

  const changeFxValuePath = (code: string, path: string) => {
    setFxValuePaths((current) => ({ ...current, [code]: path }));
    setTestResult(null);
  };

  const buildConfig = () => {
    const auth = { auth_type: authType, ...(authType === "header" || authType === "query" ? { api_key_name: apiKeyName.trim() } : {}) };
    if (mode === "fx") {
      if (provider === "frankfurter") return { provider: "frankfurter", base: base.toUpperCase(), quote: quote.toUpperCase() };
      const common = { provider: "generic_json", url: url.trim(), base: base.toUpperCase(), response_mode: fxResponseMode, ...auth };
      if (fxResponseMode === "single") return { ...common, value_path: valuePath.trim(), quote: quote.toUpperCase() };
      if (fxResponseMode === "currency_paths") return { ...common, quotes: quotes.filter((item) => item !== base), value_paths: Object.fromEntries(quotes.filter((item) => item !== base).map((item) => [item, (fxValuePaths[item] ?? "").trim()])) };
      return {
        ...common,
        quotes: quotes.filter((item) => item !== base),
        rates_path: ratesPath.trim(),
        ...(fxResponseMode === "currency_map" ? { rate_field: rateField.trim() } : { currency_field: currencyField.trim(), rate_field: rateField.trim() }),
      };
    }
    if (mode === "crypto") {
      const assets = cryptoInstrumentIds.map((id) => ({ instrument_id: id, [provider === "coingecko_simple" ? "coin_id" : "lookup_key"]: (cryptoKeys[id] ?? "").trim() }));
      if (provider === "coingecko_simple") return { provider, assets, vs_currency: currency.toLowerCase() };
      return {
        provider: "generic_json", url: url.trim(), response_mode: priceResponseMode, assets, currency: currency.toUpperCase(), prices_path: pricesPath.trim(), ...auth,
        ...(priceResponseMode === "asset_map" ? { price_field: priceField.trim() } : { symbol_field: symbolField.trim(), price_field: priceField.trim() }),
      };
    }
    if (provider === "tencent_quote") return { provider, instrument_id: instrumentId, quote_symbol: quoteSymbol.trim(), currency: currency.toUpperCase(), market: stockMarket(selected) };
    return { provider: "generic_json", url: url.trim(), value_path: valuePath.trim(), instrument_id: instrumentId, currency: currency.toUpperCase(), ...auth };
  };

  const testConnection = async () => {
    setTesting(true);
    setTestError("");
    setTestResult(null);
    try {
      const result = await api<CollectorTestResult>("/api-collectors/test", { method: "POST", body: JSON.stringify({ collector_id: collector?.id ?? null, name: name.trim() || "连接测试", data_type: mode === "fx" ? "fx_rates" : "prices", config: buildConfig(), api_key: apiKey.trim() || null }) });
      setTestResult(result);
    } catch (error) { setTestError(errorMessage(error)); }
    finally { setTesting(false); }
  };

  return <FormShell label={collector ? "保存采集器" : "创建采集器"} submit={async () => {
    const selectedQuotes = quotes.filter((item) => item !== base);
    if (multiFx && selectedQuotes.length === 0) throw new Error("请至少选择一个不同于基础币种的报价币种");
    if (mode === "fx" && generic && fxResponseMode === "currency_paths" && selectedQuotes.some((item) => !(fxValuePaths[item] ?? "").trim())) throw new Error("每个报价币种都必须填写对应的数值字段路径");
    if (multiCrypto && cryptoInstrumentIds.length === 0) throw new Error("请至少选择一个虚拟货币或稳定币");
    const selectedCryptoKeys = cryptoInstrumentIds.map((id) => (cryptoKeys[id] ?? "").trim());
    if (multiCrypto && selectedCryptoKeys.some((key) => !key)) throw new Error(provider === "coingecko_simple" ? "每个币种都必须填写 CoinGecko Coin ID" : "每个币种都必须填写接口识别键");
    if (multiCrypto && new Set(selectedCryptoKeys.map((key) => key.toLowerCase())).size !== selectedCryptoKeys.length) throw new Error("币种接口识别键不能重复");
    const defaultName = mode === "fx" ? multiFx ? `${base} 多币种汇率（${selectedQuotes.length} 项）` : `${base}/${quote} 公开汇率` : mode === "crypto" ? `虚拟货币与稳定币行情（${cryptoInstrumentIds.length} 项）` : `${selected?.symbol ?? "标的"} API 行情`;
    const savedCollector = await api<Collector>(collector ? `/api-collectors/${collector.id}` : "/api-collectors", { method: collector ? "PUT" : "POST", body: JSON.stringify({ name: name.trim() || defaultName, source_type: mode === "fx" ? "fx" : "market_data", priority, config: buildConfig(), data_type: mode === "fx" ? "fx_rates" : "prices", interval_seconds: minutes * 60, timezone: data.settings.timezone, is_enabled: enabled, api_key: apiKey.trim() || null, clear_api_key: clearApiKey }) });
    if (!runNow) { await saved(collector ? "API 采集器已更新" : "API 采集器已创建"); return; }
    const run = await api<SyncRun>(`/api-collectors/${savedCollector.id}/run`, { method: "POST" });
    await saved(run.status === "succeeded" ? "采集器已保存并成功获取数据" : `采集器已保存，本次获取失败：${run.error_message ?? "未知错误"}`);
  }}>
    <div className="api-editor-banner"><i>API</i><span><strong>配置、认证、测试、调度一体化</strong><small>测试连接不会写入价格或汇率；保存后才进入自动采集计划。</small></span>{collector?.has_api_key && !clearApiKey && <b>🔑 已配置密钥</b>}</div>
    <section className="api-editor-section"><h3>1. 数据目标</h3><label>采集器名称<input value={name} onChange={(event) => setName(event.target.value)} placeholder={mode === "fx" ? multiFx ? `${base} 多币种汇率接口` : `${base}/${quote} 汇率接口` : mode === "crypto" ? `虚拟货币与稳定币批量行情` : `${selected?.symbol ?? "标的"} API 行情`} /></label><div className="collector-kind"><button type="button" className={mode === "price" ? "active" : ""} onClick={() => { setMode("price"); if (valuePath === "rate") setValuePath("price"); setProvider(stockMarket(selected) ? "tencent_quote" : "generic_json"); if (!collector) setMinutes(stockMarket(selected) ? 5 : 60); }}>市场价格</button><button type="button" className={mode === "crypto" ? "active" : ""} onClick={() => { if (!collector || !["coingecko_simple", "generic_json"].includes(provider)) setProvider("coingecko_simple"); setMode("crypto"); if (!cryptoInstrumentIds.length && cryptoInstruments[0]) setCryptoInstrumentIds([cryptoInstruments[0].id]); setCurrency("USD"); if (!collector) setMinutes(15); }}>虚拟货币 / 稳定币</button><button type="button" className={mode === "fx" ? "active" : ""} onClick={() => { setMode("fx"); if (valuePath === "price") setValuePath("rate"); if (!["frankfurter", "generic_json"].includes(provider)) setProvider("frankfurter"); if (!collector) setMinutes(1440); }}>外汇汇率</button></div>{mode === "price" ? <label>需要获取价格的标的<select value={instrumentId} onChange={(event) => changeInstrument(event.target.value)} required><option value="">请选择标的</option>{data.instruments.filter((item) => item.asset_type !== "cash" && !["crypto", "stablecoin"].includes(item.asset_type)).map((item) => <option key={item.id} value={item.id}>{item.symbol} · {item.name} · {item.currency}</option>)}</select></label> : mode === "crypto" ? <fieldset className="multi-choice crypto-asset-choice"><legend>导入虚拟货币与稳定币（已选择 {cryptoInstrumentIds.length} 项）</legend><div>{cryptoInstruments.map((instrument) => <button type="button" key={instrument.id} className={cryptoInstrumentIds.includes(instrument.id) ? "active" : ""} onClick={() => toggleCryptoInstrument(instrument)}><i>{cryptoInstrumentIds.includes(instrument.id) ? "✓" : "+"}</i><span><strong>{instrument.symbol}</strong><small>{assetLabels[instrument.asset_type]} · {instrument.name}</small></span></button>)}</div><small>一个采集器可以同时获取主流虚拟货币和稳定币价格。</small></fieldset> : <div className="form-grid"><label>基础币种<MajorCurrencySelect value={base} onChange={(next) => { setBase(next); setQuotes((current) => current.filter((item) => item !== next)); if (quote === next) setQuote(majorCurrencies.find((item) => item.code !== next)?.code ?? "USD"); setTestResult(null); }} /></label>{!multiFx && <label>报价币种<MajorCurrencySelect value={quote} onChange={setQuote} /></label>}</div>}</section>
    <section className="api-editor-section"><h3>2. 接口与映射</h3><label>数据接口<select value={provider} onChange={(event) => { const next = event.target.value; setProvider(next); setTestResult(null); setTestError(""); if (next === "tencent_quote") { setQuoteSymbol(stockQuoteSymbol(selected)); setMinutes(5); } if (mode === "fx" && next === "generic_json" && !collector) setFxResponseMode("currency_paths"); if (mode === "crypto") setCryptoKeys((current) => Object.fromEntries(cryptoInstruments.map((instrument) => [instrument.id, next === "coingecko_simple" ? defaultCoinGeckoId(instrument) : current[instrument.id] || instrument.symbol]))); }} >{mode === "fx" ? <><option value="frankfurter">Frankfurter / ECB（无需密钥）</option><option value="generic_json">自定义 JSON 汇率 API</option></> : mode === "crypto" ? <><option value="coingecko_simple">CoinGecko（批量虚拟货币）</option><option value="generic_json">自定义 JSON 虚拟货币 API</option></> : <><option value="tencent_quote">腾讯公开行情（美股、港股）</option><option value="generic_json">自定义 JSON API（基金、黄金等）</option></>}</select></label>{mode === "fx" && generic && <label>接口响应结构<select value={fxResponseMode} onChange={(event) => { const next = event.target.value as FxResponseMode; setFxResponseMode(next); if (next === "currency_list" && !rateField) setRateField("rate"); setTestResult(null); setTestError(""); }}><option value="single">单一货币汇率</option><option value="currency_paths">多货币独立字段路径（逐币种配置）</option><option value="currency_map">多货币对象（货币代码作为键）</option><option value="currency_list">多货币数组（每项一条记录）</option></select></label>}{mode === "crypto" && generic && <label>接口响应结构<select value={priceResponseMode} onChange={(event) => { const next = event.target.value as PriceResponseMode; setPriceResponseMode(next); if (next === "asset_list" && !priceField) setPriceField("price"); setTestResult(null); setTestError(""); }}><option value="asset_map">多币种对象（币种代码作为键）</option><option value="asset_list">多币种数组（每项一条记录）</option></select></label>}
      {mode === "crypto" ? <><label>统一价格币种<MajorCurrencySelect value={currency} onChange={setCurrency} /></label><div className="crypto-api-mapping"><div className="mapping-head"><strong>{provider === "coingecko_simple" ? "CoinGecko Coin ID" : "接口识别键"}</strong><small>{cryptoInstrumentIds.length} 个已选标的</small></div>{cryptoInstrumentIds.map((id) => { const instrument = cryptoInstruments.find((item) => item.id === id); if (!instrument) return null; return <label key={id}><span><b>{instrument.symbol}</b><small>{assetLabels[instrument.asset_type]} · {instrument.name}</small></span><input value={cryptoKeys[id] ?? ""} onChange={(event) => changeCryptoKey(id, event.target.value)} placeholder={provider === "coingecko_simple" ? defaultCoinGeckoId(instrument) : instrument.symbol} required /></label>; })}</div>{provider === "coingecko_simple" ? <p className="form-help">系统已为主流币种预填 CoinGecko Coin ID；可按实际 CoinGecko 标识修改。一次请求会获取全部已选币种。</p> : <><label>HTTPS API 地址<input type="url" value={url} onChange={(event) => setUrl(event.target.value)} placeholder="https://api.example.com/prices?symbols={symbols}" required /></label><label>价格集合路径<input value={pricesPath} onChange={(event) => setPricesPath(event.target.value)} placeholder="data.prices；根对象填写 $" required /></label>{priceResponseMode === "asset_map" ? <label>对象值内的价格字段（可选）<input value={priceField} onChange={(event) => setPriceField(event.target.value)} placeholder="直接为数值时留空；嵌套对象可填 price" /></label> : <div className="form-grid"><label>币种代码字段<input value={symbolField} onChange={(event) => setSymbolField(event.target.value)} placeholder="symbol / code" required /></label><label>价格数值字段<input value={priceField} onChange={(event) => setPriceField(event.target.value)} placeholder="price / last" required /></label></div>}<p className="form-help">对象模式读取类似 data.prices.BTC 的值；数组模式按币种代码字段匹配记录。URL 可使用逗号分隔的 {`{symbols}`} 占位符。</p></>}</> : mode === "price" && provider === "tencent_quote" ? <><div className="form-grid"><label>行情代码<input value={quoteSymbol} onChange={(event) => setQuoteSymbol(event.target.value)} placeholder={selected?.currency === "HKD" ? "hk00700" : "usAAPL"} required /></label><label>价格币种<MajorCurrencySelect value={currency} onChange={setCurrency} /></label></div><p className="form-help">美股使用 usAAPL；港股使用 hk00700。系统会为 USD/HKD 股票和 ETF 自动生成。</p></> : generic ? <><label>HTTPS API 地址<input type="url" value={url} onChange={(event) => setUrl(event.target.value)} placeholder={mode === "fx" ? multiFx ? "https://api.example.com/latest?base={base}&symbols={quotes}" : "https://api.example.com/rate?base={base}&quote={quote}" : "https://api.example.com/quote"} required /></label>{multiFx && <fieldset className="multi-choice fx-currency-choice"><legend>导入报价币种（已选择 {quotes.length} 项）</legend><div>{majorCurrencies.filter((item) => item.code !== base).map((item) => <button type="button" key={item.code} className={quotes.includes(item.code) ? "active" : ""} onClick={() => toggleFxQuote(item.code)}><i>{quotes.includes(item.code) ? "✓" : "+"}</i>{item.code} · {item.label}</button>)}</div><small>选择后，下方会为每个报价币种生成一条独立配置。</small></fieldset>}{multiFx ? fxResponseMode === "currency_paths" ? <><div className="crypto-api-mapping fx-path-mapping"><div className="mapping-head"><strong>报价币种与数值字段路径</strong><small>{quotes.length} 项一一对应</small></div>{quotes.map((code) => <label key={code}><span><b>{base} / {code}</b><small>{majorCurrencies.find((item) => item.code === code)?.label}</small></span><input value={fxValuePaths[code] ?? ""} onChange={(event) => changeFxValuePath(code, event.target.value)} placeholder={`data.rates.${code}`} required /></label>)}</div><p className="form-help">每个路径都从同一次 API 返回的 JSON 根节点开始，例如 CNY 填 data.china.rate，EUR 填 payload.eur.value。</p></> : <><label>汇率集合路径<input value={ratesPath} onChange={(event) => setRatesPath(event.target.value)} placeholder="data.rates；根对象填写 $" required /></label>{fxResponseMode === "currency_map" ? <label>对象值内的汇率字段（可选）<input value={rateField} onChange={(event) => setRateField(event.target.value)} placeholder="直接为数值时留空；嵌套对象可填 rate" /></label> : <div className="form-grid"><label>货币代码字段<input value={currencyField} onChange={(event) => setCurrencyField(event.target.value)} placeholder="currency / code" required /></label><label>汇率数值字段<input value={rateField} onChange={(event) => setRateField(event.target.value)} placeholder="rate / mid" required /></label></div>}<p className="form-help">对象模式读取类似 data.rates.CNY 的值；数组模式读取每条记录中的货币代码和汇率字段。URL 支持 {`{base}`} 和逗号分隔的 {`{quotes}`} 占位符。</p></> : <><div className="form-grid"><label>数值字段路径<input value={valuePath} onChange={(event) => setValuePath(event.target.value)} placeholder={mode === "fx" ? "data.rate" : "data.price"} required /></label>{mode === "price" && <label>价格币种<MajorCurrencySelect value={currency} onChange={setCurrency} /></label>}</div><p className="form-help">支持点号和数组下标，例如 data.rates.0.value。单一汇率 URL 可使用 {`{base}`} 和 {`{quote}`} 占位符；只允许公网 HTTPS。</p></>}</> : <p className="form-help">Frankfurter 使用欧洲央行参考汇率，无需 API Key。点击下方“测试连接”可先确认返回值。</p>}
    </section>
    {generic && <section className="api-editor-section api-auth-section"><h3>3. API 认证</h3><div className="form-grid"><label>认证方式<select value={authType} onChange={(event) => { const next = event.target.value; setAuthType(next); if (next === "header" && !apiKeyName) setApiKeyName("X-API-Key"); if (next === "query" && !apiKeyName) setApiKeyName("apikey"); }}><option value="none">不需要 API Key</option><option value="header">请求头 Header</option><option value="query">URL 查询参数</option><option value="bearer">Bearer Token</option></select></label>{(authType === "header" || authType === "query") && <label>{authType === "header" ? "请求头名称" : "参数名称"}<input value={apiKeyName} onChange={(event) => setApiKeyName(event.target.value)} placeholder={authType === "header" ? "X-API-Key" : "apikey"} required /></label>}</div>{authType !== "none" && <><label>API Key<input type="password" value={apiKey} onChange={(event) => { setApiKey(event.target.value); setClearApiKey(false); }} placeholder={collector?.has_api_key ? "已加密保存；留空则保持原密钥" : "输入 API Key 或 Token"} autoComplete="new-password" /></label>{collector?.has_api_key && <label className="check"><input type="checkbox" checked={clearApiKey} onChange={(event) => { setClearApiKey(event.target.checked); if (event.target.checked) setApiKey(""); }} />删除已经保存的 API Key</label>}<p className="credential-note">🔒 API Key 与普通配置分开加密保存，不会显示在采集器列表、运行日志或接口响应中。</p></>}</section>}
    <section className="api-editor-section"><h3>{generic ? "4" : "3"}. 测试与调度</h3><div className="api-test-row"><button type="button" className="soft compact" disabled={testing} onClick={() => void testConnection()}>{testing ? "正在测试…" : "测试连接与字段映射"}</button><span>只读取一次，不写入账本</span></div>{testError && <div className="api-test-result error"><strong>测试失败</strong><span>{testError}</span></div>}{testResult && <div className="api-test-result ok"><div><strong>✓ 接口测试成功 · {testResult.record_count} 条记录</strong><span>{testResult.elapsed_ms} ms · {testResult.provider}{testResult.used_api_key ? " · 已使用 API Key" : ""}</span></div><code>{testResult.request_url}</code><pre>{JSON.stringify(testResult.normalized_preview, null, 2)}</pre></div>}<div className="form-grid three"><label>优先级<input type="number" min="0" max="10000" value={priority} onChange={(event) => setPriority(Number(event.target.value))} /></label><label>刷新间隔（分钟）<input type="number" min="1" max="525600" value={minutes} onChange={(event) => setMinutes(Number(event.target.value))} required /></label><label>写入位置<input value={mode === "fx" ? multiFx ? `${quotes.length} 个币种的汇率账本` : "汇率账本" : mode === "crypto" ? `${cryptoInstrumentIds.length} 个币种的价格历史` : `${selected?.symbol ?? "标的"} 价格历史`} disabled /></label></div><div className="form-grid"><label className="check"><input type="checkbox" checked={enabled} onChange={(event) => setEnabled(event.target.checked)} />启用自动采集</label><label className="check"><input type="checkbox" checked={runNow} onChange={(event) => setRunNow(event.target.checked)} />保存后立即获取一次</label></div></section>
    {collector && <button type="button" className="delete-collector" onClick={async () => { if (!window.confirm(`确认删除采集器“${collector.name}”吗？历史运行记录会保留。`)) return; await api(`/api-collectors/${collector.id}`, { method: "DELETE" }); await saved("API 采集器已删除"); }}>删除此采集器</button>}
    <div className="correction-note">采集器仅执行只读 GET 请求，不具备下单、转账或提币权限。删除采集器后历史运行记录仍会保留。</div>
  </FormShell>;
}

function AppearanceSettings({ value, onChange }: { value: Appearance; onChange: (appearance: Appearance) => void }) {
  return <section className="card theme-card"><div className="section-head"><div><p>个性化外观</p><h3>页面模式与背景</h3></div><span className="theme-current">{value.mode === "dark" ? "暗色" : "白色"} · {appearanceThemes.find((item) => item.value === value.theme)?.label}</span></div><div className="theme-group"><strong>页面模式</strong><div className="theme-mode-picker"><button type="button" className={value.mode === "light" ? "active" : ""} aria-pressed={value.mode === "light"} onClick={() => onChange({ ...value, mode: "light" })}><i className="light-mode-preview"><span /></i><span><b>白色页面</b><small>明亮卡片与柔和背景</small></span></button><button type="button" className={value.mode === "dark" ? "active" : ""} aria-pressed={value.mode === "dark"} onClick={() => onChange({ ...value, mode: "dark" })}><i className="dark-mode-preview"><span /></i><span><b>暗色页面</b><small>深色画布与低亮度卡片</small></span></button></div></div><div className="theme-group"><strong>背景配色</strong><div className="theme-palette-picker">{appearanceThemes.map((theme) => <button type="button" key={theme.value} className={value.theme === theme.value ? "active" : ""} aria-pressed={value.theme === theme.value} onClick={() => onChange({ ...value, theme: theme.value })}><i style={{ background: `linear-gradient(135deg,${theme.colors[0]} 0 56%,${theme.colors[1]} 56% 100%)` }} /><span>{theme.label}</span><b>{value.theme === theme.value ? "✓" : ""}</b></button>)}</div></div><p className="theme-help">选择后立即应用，并保存在当前设备；不会改变投资数据或导出内容。</p></section>;
}

function NetworkProxyForm({ value, onSaved }: { value: NetworkProxy; onSaved: (message: string) => Promise<void> }) {
  const [enabled, setEnabled] = useState(value.is_enabled);
  const [protocol, setProtocol] = useState<NetworkProxy["protocol"]>(value.protocol);
  const [host, setHost] = useState(value.host);
  const [port, setPort] = useState(value.port);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ kind: "ok" | "error"; message: string } | null>(null);
  const payload = () => ({ is_enabled: enabled, protocol, host: host.trim(), port });
  const testConnection = async () => {
    setTesting(true); setTestResult(null);
    try {
      const result = await api<{ ok: boolean; mode: string; status: number; latency_ms: number }>("/network-proxy/test", { method: "POST", body: JSON.stringify(payload()) });
      setTestResult({ kind: "ok", message: `${enabled ? "代理" : "直连"}可用 · HTTP ${result.status} · ${result.latency_ms} ms` });
    } catch (error) { setTestResult({ kind: "error", message: errorMessage(error) }); }
    finally { setTesting(false); }
  };
  return <section className="card proxy-card"><div className="section-head"><div><p>网络协议</p><h3>外部数据代理</h3></div><span className={enabled ? "proxy-badge on" : "proxy-badge"}><i />{enabled ? "代理已启用" : "当前直连"}</span></div><FormShell label="保存代理设置" submit={async () => { await api("/network-proxy", { method: "PUT", body: JSON.stringify(payload()) }); await onSaved(enabled ? "网络代理已启用" : "已切换为网络直连"); }}><div className="proxy-mode"><button type="button" className={!enabled ? "active" : ""} onClick={() => { setEnabled(false); setTestResult(null); }}><i>↗</i><span><strong>直接连接</strong><small>外部 API 不经过代理</small></span></button><button type="button" className={enabled ? "active" : ""} onClick={() => { setEnabled(true); setTestResult(null); }}><i>⇄</i><span><strong>使用代理</strong><small>行情、汇率与虚拟货币 API</small></span></button></div><div className="form-grid three"><label>代理协议<select value={protocol} onChange={(event) => setProtocol(event.target.value as NetworkProxy["protocol"])} disabled={!enabled}><option value="http">HTTP</option><option value="https">HTTPS</option><option value="socks5">SOCKS5</option></select></label><label>代理主机<input value={host} onChange={(event) => setHost(event.target.value)} placeholder="127.0.0.1" disabled={!enabled} required /></label><label>端口<input type="number" min="1" max="65535" value={port} onChange={(event) => setPort(Number(event.target.value))} disabled={!enabled} required /></label></div><p className="form-help">代理仅用于后端访问公开行情、汇率、CoinGecko 和通用 API；localhost 本地连接不受影响。SOCKS5 会通过代理解析目标域名。</p><div className="proxy-test"><button type="button" className="soft compact" disabled={testing} onClick={() => void testConnection()}>{testing ? "正在测试…" : "测试当前配置"}</button>{testResult && <span className={testResult.kind}>{testResult.kind === "ok" ? "✓" : "!"} {testResult.message}</span>}</div></FormShell></section>;
}

function CurrencySettingsForm({ value, onSaved }: { value: Settings; onSaved: (message: string) => Promise<void> }) {
  const [currency, setCurrency] = useState(normalizeMajorCurrency(value.report_currency));
  const [timezone, setTimezone] = useState(timezoneOptions.some((item) => item.value === value.timezone) ? value.timezone : "Asia/Shanghai");
  const [method, setMethod] = useState(value.cost_method);
  const [stale, setStale] = useState(value.stale_price_days);
  const [absolute, setAbsolute] = useState(value.absolute_rebalance_threshold);
  const [relative, setRelative] = useState(value.relative_rebalance_threshold);
  const [hardDeleteMinutes, setHardDeleteMinutes] = useState(value.transaction_hard_delete_minutes);
  return <section className="card"><div className="section-head"><div><p>组合口径</p><h3>估值、再平衡与纠错</h3></div></div><FormShell label="保存组合设置" submit={async () => { await api("/settings", { method: "PUT", body: JSON.stringify({ report_currency: currency, timezone, cost_method: method, stale_price_days: stale, absolute_rebalance_threshold: absolute, relative_rebalance_threshold: relative, transaction_hard_delete_minutes: hardDeleteMinutes }) }); await onSaved("组合设置与流水纠错时限已保存"); }}><div className="form-grid"><label>核心报告币种<MajorCurrencySelect value={currency} onChange={setCurrency} /></label><label>日结时区<select value={timezone} onChange={(e) => setTimezone(e.target.value)}>{timezoneOptions.map((item) => <option key={item.value} value={item.value}>{item.label} · {item.value}</option>)}</select></label></div><div className="form-grid"><label>成本法<select value={method} onChange={(e) => setMethod(e.target.value)}><option value="average">移动加权平均</option><option value="fifo">FIFO（预留）</option></select></label><label>价格陈旧天数<input type="number" min="0" value={stale} onChange={(e) => setStale(Number(e.target.value))} /></label></div><div className="form-grid"><label>绝对偏离阈值<input value={absolute} onChange={(e) => setAbsolute(e.target.value)} /></label><label>相对偏离阈值<input value={relative} onChange={(e) => setRelative(e.target.value)} /></label></div><div className="correction-window-setting"><span><strong>误录流水纠错时限</strong><small>时限内可彻底删除刚刚手工录入且尚未编辑或撤销的流水；超过时限只能撤销。</small></span><label>允许彻底删除<select value={hardDeleteMinutes} onChange={(event) => setHardDeleteMinutes(Number(event.target.value))}><option value={0}>关闭</option><option value={5}>5 分钟</option><option value={15}>15 分钟</option><option value={30}>30 分钟</option><option value={60}>1 小时</option><option value={180}>3 小时</option><option value={1440}>24 小时</option><option value={10080}>7 天</option></select></label></div></FormShell></section>;
}
function PolicyForm({ value, onSaved }: { value: Policy; onSaved: (message: string) => Promise<void> }) {
  const [policy, setPolicy] = useState(value);
  const update = (patch: Partial<Policy>) => setPolicy((current) => ({ ...current, ...patch }));
  const toggleTool = (field: "allowed_tools" | "prohibited_tools", otherField: "allowed_tools" | "prohibited_tools", tool: string) => setPolicy((current) => {
    const selected = splitSelections(current[field]);
    const adding = !selected.includes(tool);
    const next = adding ? [...selected, tool] : selected.filter((item) => item !== tool);
    const other = adding ? splitSelections(current[otherField]).filter((item) => item !== tool) : splitSelections(current[otherField]);
    return { ...current, [field]: next.join(","), [otherField]: other.join(",") };
  });
  return <section className="card"><div className="section-head"><div><p>投资政策声明</p><h3>纪律与风险边界</h3></div></div><FormShell label="保存 IPS" submit={async () => { await api("/policy", { method: "PUT", body: JSON.stringify(policy) }); await onSaved("投资政策已保存"); }}><label>投资目标<textarea value={policy.objective} onChange={(e) => update({ objective: e.target.value })} /></label><div className="form-grid"><label>投资期限（年）<input type="number" value={policy.horizon_years} onChange={(e) => update({ horizon_years: Number(e.target.value) })} /></label><label>应急资金（月）<input type="number" value={policy.emergency_fund_months} onChange={(e) => update({ emergency_fund_months: Number(e.target.value) })} /></label></div><div className="form-grid three"><label>最大回撤<input value={policy.max_drawdown} onChange={(e) => update({ max_drawdown: e.target.value })} /></label><label>单一标的上限<input value={policy.max_single_position} onChange={(e) => update({ max_single_position: e.target.value })} /></label><label>高风险上限<input value={policy.max_high_risk} onChange={(e) => update({ max_high_risk: e.target.value })} /></label></div><MultiChoice label="允许工具" values={splitSelections(policy.allowed_tools)} options={investmentToolOptions} onToggle={(tool) => toggleTool("allowed_tools", "prohibited_tools", tool)} help="可多选；加入允许清单时，会自动从禁止清单移除。" /><MultiChoice label="禁止工具" values={splitSelections(policy.prohibited_tools)} options={investmentToolOptions} onToggle={(tool) => toggleTool("prohibited_tools", "allowed_tools", tool)} help="可多选；同一工具不会同时出现在允许和禁止清单。" /><div className="form-grid"><label>再平衡频率<select value={policy.rebalance_frequency} onChange={(e) => update({ rebalance_frequency: e.target.value })}>{rebalanceFrequencyOptions.map((item) => <option key={item.value} value={item.value}>{item.label}</option>)}</select></label><label>复盘频率<select value={policy.review_frequency} onChange={(e) => update({ review_frequency: e.target.value })}>{reviewFrequencyOptions.map((item) => <option key={item.value} value={item.value}>{item.label}</option>)}</select></label></div></FormShell></section>;
}

function Metric({ label, value, detail }: { label: string; value: string; detail: string }) { return <div className="card metric"><span>{label}</span><strong>{value}</strong><small>{detail}</small></div>; }
function Risk({ label, value }: { label: string; value: string }) { return <div><span>{label}</span><strong>{value}</strong></div>; }
function AllocationBars({ items }: { items: Allocation[] }) { return <div className="allocation-bars">{items.length ? items.slice(0, 6).map((item, index) => <div key={item.key}><span><i style={{ background: ["#1677ff", "#13a8d8", "#725cff", "#2bbbad", "#4f8ddf", "#647b9b"][index % 6] }} />{assetLabels[item.key] ?? item.key}</span><div><i style={{ width: `${Math.min(100, number(item.weight) * 100)}%` }} /></div><strong>{percent(item.weight)}</strong></div>) : <p className="note">录入流水和价格后显示真实配置。</p>}</div>; }

const chartColors = ["#1677ff", "#36b8e8", "#7357e8", "#22a99a", "#e8a23a", "#6b88a7", "#d56868"];

function AllocationDonut({ items, total, currency }: { items: Allocation[]; total: string; currency: string }) {
  const sorted = [...items].filter((item) => number(item.weight) > 0).sort((a, b) => number(b.weight) - number(a.weight));
  const remainder = sorted.slice(5);
  const shown = remainder.length ? [...sorted.slice(0, 5), { key: "other", value: String(remainder.reduce((sum, item) => sum + number(item.value), 0)), weight: String(remainder.reduce((sum, item) => sum + number(item.weight), 0)) }] : sorted;
  const stops = shown.map((item, index) => { const start = shown.slice(0, index).reduce((sum, previous) => sum + number(previous.weight) * 100, 0); const end = start + number(item.weight) * 100; return `${chartColors[index % chartColors.length]} ${start}% ${end}%`; });
  const covered = shown.reduce((sum, item) => sum + number(item.weight) * 100, 0);
  const background = stops.length ? `conic-gradient(${stops.join(",")}${covered < 100 ? `,#e7eef6 ${covered}% 100%` : ""})` : "#e7eef6";
  return <div className={`allocation-donut-panel ${shown.length ? "" : "is-empty"}`}><div className="donut-wrap" role="img" aria-label={`组合资产配置，共 ${shown.length} 类`}><div className="allocation-donut" style={{ background }}><div><small>总资产</small><strong>{money(total, currency)}</strong><span>{shown.length} 类配置</span></div></div></div><div className="donut-legend">{shown.map((item, index) => <div key={item.key}><i style={{ background: chartColors[index % chartColors.length] }} /><span><strong>{item.key === "other" ? "其他" : assetLabels[item.key] ?? item.key}</strong><small>{money(item.value, currency)}</small></span><b>{percent(item.weight)}</b></div>)}{!shown.length && <p>录入持仓和价格后显示配置结构。</p>}</div></div>;
}

function PositionMap({ holdings, currency }: { holdings: Holding[]; currency: string }) {
  const sorted = [...holdings].filter((holding) => number(holding.market_value) > 0).sort((a, b) => number(b.market_value) - number(a.market_value));
  const visible = sorted.slice(0, 5).map((holding) => ({ key: `${holding.account_id}-${holding.instrument_id}`, symbol: holding.symbol, name: holding.name, value: number(holding.market_value), weight: number(holding.weight) }));
  const otherValue = sorted.slice(5).reduce((sum, holding) => sum + number(holding.market_value), 0);
  const otherWeight = sorted.slice(5).reduce((sum, holding) => sum + number(holding.weight), 0);
  const slices = otherValue > 0 ? [...visible, { key: "other", symbol: "其他", name: `${sorted.length - 5} 个持仓`, value: otherValue, weight: otherWeight }] : visible;
  if (!slices.length) return <Empty title="录入流水和价格后生成持仓权重图" />;
  return <div className="position-map"><div className="position-stack" role="img" aria-label="按组合权重排列的持仓地图">{slices.map((slice, index) => <div key={slice.key} style={{ flexGrow: Math.max(slice.value, 0.0001), background: chartColors[index % chartColors.length] }} title={`${slice.symbol} · ${percent(slice.weight)} · ${money(slice.value, currency)}`}><strong>{slice.weight >= .08 ? slice.symbol : ""}</strong><small>{slice.weight >= .13 ? percent(slice.weight) : ""}</small></div>)}</div><div className="position-list">{slices.map((slice, index) => <div key={slice.key}><i style={{ background: chartColors[index % chartColors.length] }}>{index + 1}</i><span><strong>{slice.symbol}</strong><small>{slice.name}</small></span><div><span style={{ width: `${Math.min(100, slice.weight * 100)}%`, background: chartColors[index % chartColors.length] }} /></div><b>{percent(slice.weight)}</b><em>{money(slice.value, currency)}</em></div>)}</div></div>;
}

function RiskCockpit({ risk, maxSingle, maxHighRisk }: { risk: Portfolio["risk"]; maxSingle: number; maxHighRisk: number }) {
  const gauges = [{ label: "最大单一仓位", value: number(risk.max_position_weight), threshold: maxSingle || undefined }, { label: "虚拟货币", value: number(risk.crypto_weight), threshold: maxHighRisk || undefined }, { label: "现金比例", value: number(risk.cash_weight) }, { label: "账户集中度", value: number(risk.account_concentration) }];
  return <div className="risk-cockpit">{gauges.map((gauge) => { const capped = Math.min(1, Math.max(0, gauge.value)); const caution = gauge.threshold !== undefined && gauge.value > gauge.threshold; const color = caution ? "#d56868" : "#1677ff"; return <div key={gauge.label}><div className="risk-dial" style={{ background: `conic-gradient(${color} ${capped * 360}deg,#e7eef6 0deg)` }} role="img" aria-label={`${gauge.label} ${percent(gauge.value)}`}><span><strong>{percent(gauge.value)}</strong><small>{caution ? "已越界" : gauge.threshold ? `上限 ${percent(gauge.threshold)}` : "当前"}</small></span></div><b>{gauge.label}</b></div>; })}<div className="risk-foot"><span><i className={risk.missing_price_count + risk.stale_price_count ? "warn" : "ok"} />价格质量 <b>{risk.missing_price_count + risk.stale_price_count ? `${risk.missing_price_count + risk.stale_price_count} 项异常` : "完整"}</b></span><span><i className={risk.target_breaches.length ? "warn" : "ok"} />配置边界 <b>{risk.target_breaches.length ? `${risk.target_breaches.length} 项越界` : "正常"}</b></span></div></div>;
}

function ExposureExplorer({ currency, byCurrency, byAccount }: { currency: string; byCurrency: Allocation[]; byAccount: Allocation[] }) {
  const [dimension, setDimension] = useState<"currency" | "account">("currency");
  const items = [...(dimension === "currency" ? byCurrency : byAccount)].sort((a, b) => number(b.weight) - number(a.weight));
  const leader = items[0];
  return <div className="exposure-explorer"><div className="section-head"><div><p>敞口分析</p><h3>{dimension === "currency" ? "币种分布" : "账户分布"}</h3></div><div className="chart-tabs"><button className={dimension === "currency" ? "active" : ""} onClick={() => setDimension("currency")}>按币种</button><button className={dimension === "account" ? "active" : ""} onClick={() => setDimension("account")}>按账户</button></div></div>{leader && <div className="exposure-summary"><span>最大敞口<strong>{leader.key}</strong></span><b>{percent(leader.weight)}</b><small>{money(leader.value, currency)}</small></div>}<div className="exposure-bars">{items.slice(0, 7).map((item, index) => <div key={item.key}><span><i style={{ background: chartColors[index % chartColors.length] }} />{item.key}</span><div><i style={{ width: `${Math.min(100, number(item.weight) * 100)}%`, background: `linear-gradient(90deg,${chartColors[index % chartColors.length]},${chartColors[(index + 1) % chartColors.length]})` }} /></div><strong>{percent(item.weight)}</strong><small>{money(item.value, currency)}</small></div>)}{!items.length && <Empty title="有持仓后显示币种和账户敞口" />}</div></div>;
}
function AlertRows({ data }: { data: Snapshot }) { const items = [{ label: "缺失价格", value: data.portfolio.risk.missing_price_count }, { label: "陈旧价格", value: data.portfolio.risk.stale_price_count }, { label: "缺失汇率", value: data.portfolio.risk.missing_fx_count }, { label: "策略越界", value: data.portfolio.risk.target_breaches.length }]; return <div className="alert-list">{items.map((item) => <div key={item.label}><i className={item.value ? "warn" : "ok"}>{item.value ? "!" : "✓"}</i><span>{item.label}</span><b>{item.value}</b></div>)}</div>; }
function HoldingsTable({ holdings, currency, full = false }: { holdings: Holding[]; currency: string; full?: boolean }) { return <div className={`holdings ${full ? "full" : ""}`}><div className="table-head"><span>标的</span><span>账户</span><span>数量</span><span>市值</span><span>盈亏</span><span>权重</span></div>{holdings.map((holding) => <div className="holding-row" key={`${holding.account_id}-${holding.instrument_id}`}><span className="asset-cell"><i>{holding.symbol.slice(0, 2)}</i><span><strong>{holding.symbol}</strong><small>{holding.name}{holding.stale || holding.missing_price ? " · 价格待更新" : ""}</small></span></span><span>{holding.account_name}</span><span>{holding.quantity}</span><span>{money(holding.market_value, currency)}</span><span className={number(holding.unrealized_pnl) >= 0 ? "positive" : "negative"}>{money(holding.unrealized_pnl, currency)}</span><span>{percent(holding.weight)}</span></div>)}{!holdings.length && <Empty title="尚无真实持仓" />}</div>; }
function TransactionList({ transactions, accounts, instruments }: { transactions: Transaction[]; accounts: Account[]; instruments: Instrument[] }) { return <div className="transaction-list">{transactions.map((tx) => { const leg = tx.legs[0]; return <div key={tx.id}><i>{(transactionLabels[tx.transaction_type] ?? tx.transaction_type).slice(0, 1)}</i><span><strong>{transactionLabels[tx.transaction_type] ?? tx.transaction_type} · {instruments.find((item) => item.id === leg?.instrument_id)?.symbol ?? "—"}</strong><small>{accounts.find((item) => item.id === leg?.account_id)?.name ?? "未知账户"} · {tx.memo || "无备注"}</small></span><b>{dateText(tx.trade_at)}</b></div>; })}{!transactions.length && <Empty title="尚无流水" />}</div>; }
function LedgerRow({ tx, data, now, hardDeleteMinutes, view, edit, voidEntry, hardRemove, voiding, hardDeleting }: { tx: Transaction; data: Snapshot; now: number; hardDeleteMinutes: number; view: () => void; edit: () => void; voidEntry: () => void; hardRemove: () => void; voiding: boolean; hardDeleting: boolean }) {
  const hardDeleteDeadline = new Date(tx.created_at).getTime() + hardDeleteMinutes * 60_000;
  const remainingMinutes = Math.max(0, Math.ceil((hardDeleteDeadline - now) / 60_000));
  const canHardDelete = hardDeleteMinutes > 0 && ["manual", "web", "web_standard"].includes(tx.source) && tx.status === "confirmed" && !tx.reverses_transaction_id && remainingMinutes > 0;
  return <article><div className="ledger-main"><i>{(transactionLabels[tx.transaction_type] ?? tx.transaction_type).slice(0, 1)}</i><span><strong>{transactionLabels[tx.transaction_type] ?? tx.transaction_type}</strong><small>{tx.memo || tx.source} · {dateText(tx.trade_at)}</small></span><b>{tx.legs.length} 条分录</b><div className="ledger-actions"><button onClick={view}>查看</button><button onClick={edit}>编辑</button><button onClick={voidEntry} disabled={voiding || hardDeleting}>{voiding ? "撤销中…" : "撤销"}</button>{canHardDelete && <button className="hard-delete" title={`彻底删除窗口剩余约 ${remainingMinutes} 分钟`} onClick={hardRemove} disabled={voiding || hardDeleting}>{hardDeleting ? "删除中…" : "彻底删除"}</button>}</div></div><div className="leg-summary">{tx.legs.map((leg, index) => <span key={index}><em>{data.accounts.find((item) => item.id === leg.account_id)?.name ?? "未知账户"}</em><strong>{data.instruments.find((item) => item.id === leg.instrument_id)?.symbol ?? "?"}</strong><b className={number(leg.quantity) >= 0 ? "positive" : "negative"}>{number(leg.quantity) > 0 ? "+" : ""}{leg.quantity}</b>{leg.unit_price && <small>@ {leg.unit_price} {leg.price_currency}</small>}</span>)}</div></article>;
}
function Empty({ title, action, onAction }: { title: string; action?: string; onAction?: () => void }) { return <div className="empty"><i>◇</i><strong>{title}</strong>{action && <button onClick={onAction}>{action}</button>}</div>; }
function downloadCsvTemplate(data: Snapshot) { const account = data.accounts[0]?.id ?? "账户UUID"; const asset = data.instruments.find((item) => !["cash", "stablecoin"].includes(item.asset_type))?.id ?? "资产标的UUID"; const cash = data.instruments.find((item) => item.asset_type === "cash")?.id ?? "现金标的UUID"; const now = new Date().toISOString(); const rows = ["transaction_group,transaction_type,trade_at,account_id,instrument_id,leg_type,quantity,unit_price,price_currency,memo,external_id", `example-1,buy,${now},${account},${asset},asset,10,100,CNY,CSV示例,csv-example-1`, `example-1,buy,${now},${account},${cash},cash,-1000,,,CSV示例,csv-example-1`]; const blob = new Blob(["\uFEFF" + rows.join("\r\n")], { type: "text/csv;charset=utf-8" }); const link = document.createElement("a"); link.href = URL.createObjectURL(blob); link.download = "sanyu-invest-transactions-template.csv"; link.click(); URL.revokeObjectURL(link.href); }
