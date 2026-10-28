/**
 * ICC 市场读取接口。
 *
 * 市场接口是公开只读的（只返回 PUBLISHED），无需登录，CORS 为 *。
 * 下载由 Rust 命令 import_market_icc 完成，前端只负责列表和触发。
 * 接口基址随接口环境走（开发构建可在设置中切预发），见 lib/serverEnv.ts。
 */
import { serverEnv } from "./serverEnv";

export const MARKET_PUBLIC_BASE = "https://filter-manage-download.xyls.us.kg";

export interface MarketProfile {
  id: string;
  sha256: string;
  objectKey: string;
  title: string;
  description: string;
  panel: string;
  tags: string[];
  sizeBytes: number;
  downloadCount: number;
  coverKey: string;
  thumbKey: string;
  contributorName: string;
  createdAt: string;
  updatedAt: string;
}

export function marketAssetUrl(key: string): string {
  return key ? `${MARKET_PUBLIC_BASE}/${key}` : "";
}

export interface MarketListResult {
  items: MarketProfile[];
  total: number;
  limit: number;
  offset: number;
}

export async function fetchMarketProfiles(params: {
  q?: string;
  panel?: string;
  limit?: number;
  offset?: number;
}): Promise<MarketListResult> {
  const search = new URLSearchParams();
  if (params.q) search.set("q", params.q);
  if (params.panel) search.set("panel", params.panel);
  search.set("limit", String(params.limit ?? 30));
  search.set("offset", String(params.offset ?? 0));

  const response = await fetch(`${serverEnv().marketApiBase}/api/market/icc?${search}`, {
    headers: { Accept: "application/json" },
  });
  if (!response.ok) {
    throw new Error(`市场加载失败（HTTP ${response.status}）`);
  }
  return (await response.json()) as MarketListResult;
}
