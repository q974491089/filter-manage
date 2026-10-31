import { serverEnv } from "./serverEnv";

/**
 * 方案分享接口（market 服务 :9879 的 /api/market/shares*）。
 *
 * 匿名：没有登录，分享由不可猜的 slug 定位；ICC 文件走既有的内容寻址上传
 * 网关（与投稿共用 token 与对象存储），所以同一份文件无论被分享多少次都只
 * 存一份。
 *
 * 后端契约见 .docs/api/share.md。
 */

export interface ShareColor {
  brightness: number;
  contrast: number;
  gamma: number;
  digitalVibrance: number;
  rgbR: number;
  rgbG: number;
  rgbB: number;
}

export interface PublicShare {
  slug: string;
  title: string;
  description: string;
  color: ShareColor;
  resolution: string;
  iccName: string;
  iccSha256: string;
  sizeBytes: number;
  coverKey: string;
  thumbKey: string;
  contributorName: string;
  importCount: number;
  createdAt: string;
}

export interface CreateSharePayload {
  title: string;
  description: string;
  color: ShareColor;
  resolution: string;
  iccSha256: string;
  iccName: string;
  coverKey: string;
  thumbKey: string;
  contributorId: string;
  contributorName: string;
}

const base = () => serverEnv().marketApiBase;

/** 服务端文件库里是否已有这份 ICC（只回布尔，不泄露其它信息）。 */
export async function checkShareIcc(sha256: string): Promise<boolean> {
  const res = await fetch(`${base()}/api/market/shares/check-icc`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ sha256 }),
  });
  if (!res.ok) {
    throw new Error(`检查文件失败（HTTP ${res.status}）`);
  }
  const data = (await res.json()) as { exists?: boolean };
  return Boolean(data.exists);
}

export async function createShare(
  payload: CreateSharePayload,
): Promise<{ slug: string; item: PublicShare }> {
  const res = await fetch(`${base()}/api/market/shares`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(payload),
  });
  const data = await res.json().catch(() => null);
  if (!res.ok) {
    const message =
      (data as { message?: string } | null)?.message ||
      `创建分享失败（HTTP ${res.status}）`;
    throw new Error(message);
  }
  return data as { slug: string; item: PublicShare };
}

/** 公开详情。分享被撤销或不存在时返回 null，调用方按「链接失效」处理。 */
export async function getShare(slug: string): Promise<PublicShare | null> {
  const res = await fetch(
    `${base()}/api/market/shares/${encodeURIComponent(slug)}`,
  );
  if (res.status === 404) return null;
  if (!res.ok) throw new Error(`加载分享失败（HTTP ${res.status}）`);
  const data = (await res.json()) as { item: PublicShare };
  return data.item;
}

/** 导入计数 +1；失败静默（导入本身已在本地完成，不该因此报错）。 */
export async function reportShareImport(slug: string): Promise<void> {
  try {
    await fetch(`${base()}/api/market/shares/${encodeURIComponent(slug)}/import`, {
      method: "POST",
    });
  } catch {
    // 计数是尽力而为
  }
}

/** 从完整 URL 或裸 slug 里取出 slug。支持落地页地址、协议链接、裸分享码。 */
export function parseShareSlug(raw: string): string | null {
  const text = raw.trim();
  if (!text) return null;
  // 裸 slug：22 位 URL-safe 字符
  if (/^[A-Za-z0-9_-]{10,64}$/.test(text)) return text;
  // 协议链接：filter-manage://import?slug=xxx（一键唤起客户端）
  const proto = text.match(/^filter-manage:\/\/.*[?&]slug=([A-Za-z0-9_-]{10,64})/i);
  if (proto) return proto[1];
  // /s/<slug> 或带域名
  const match = text.match(/\/s\/([A-Za-z0-9_-]{10,64})/);
  return match ? match[1] : null;
}

/** 分享落地页地址（与服务端 domain 保持一致，客户端只负责拼）。 */
export function shareUrl(slug: string): string {
  return `${serverEnv().marketApiBase}/s/${slug}`;
}

/** 一键唤起客户端导入的协议链接（需客户端已注册协议）。 */
export function shareProtocolUrl(slug: string): string {
  return `filter-manage://import?slug=${slug}`;
}

/** 图片资产地址（与 ICC 市场同一 R2 桶）。 */
export const SHARE_ASSET_BASE = "https://filter-manage-download.xyls.us.kg";

export function shareAssetUrl(key: string): string {
  return key ? `${SHARE_ASSET_BASE}/${key}` : "";
}
