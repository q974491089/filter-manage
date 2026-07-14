// 公告纯逻辑：类型 + 有效期过滤 + 排序 + 已读 localStorage 读写。
// 无 React 依赖，便于将来单测。字段与 src-tauri/src/announcements.rs 一一对应。

export type AnnouncementLevel = "normal" | "important";

export interface Announcement {
  id: string;
  rowId?: string;               // 库内 UUID，前端一般忽略
  type?: string;                // 渠道：client / static / web / 自定义。桌面端默认只拿 client
  title: string;
  body: string;                 // Markdown
  level: AnnouncementLevel;
  pinned?: boolean;             // 是否置顶：置顶项排最前，并在标题旁渲染「置顶」tag
  sortOrder?: number;           // 排序权重，越小越靠前（同 type 内）；置顶用小值（0/10/20…）
  publishedAt: string;          // ISO8601 UTC
  startAt?: string | null;      // 可选，缺省=立即生效
  endAt?: string | null;        // 可选，缺省=永不过期
  createdAt?: string | null;    // 可选元数据
  updatedAt?: string | null;    // 可选元数据
}

/** 格式化 ISO8601 为本地日期；无法解析返回空串。UI 展示用。 */
export function fmtDate(iso: string): string {
  const t = Date.parse(iso);
  if (Number.isNaN(t)) return "";
  return new Date(t).toLocaleDateString();
}

/** 将简易 Markdown 压成单行纯文本，用于列表两行截断预览（不渲染标记）。 */
export function plainPreview(md: string): string {
  return md
    .replace(/```[\s\S]*?```/g, " ")          // 代码块
    .replace(/`([^`]+)`/g, "$1")              // 行内代码
    .replace(/!\[[^\]]*\]\([^)]*\)/g, " ")     // 图片
    .replace(/\[([^\]]*)\]\([^)]*\)/g, "$1")   // 链接保留文字
    .replace(/^#{1,6}\s+/gm, "")              // 标题井号
    .replace(/(\*\*|__)(.*?)\1/g, "$2")        // 加粗
    .replace(/(\*|_)(.*?)\1/g, "$2")           // 斜体
    .replace(/^\s*[-*+]\s+/gm, "")             // 无序列表符号
    .replace(/^\s*>\s?/gm, "")                 // 引用
    .replace(/\s+/g, " ")                      // 合并空白
    .trim();
}

/** 是否在有效期窗口内（startAt/endAt 缺省或无法解析视为不限） */
export function isActive(a: Announcement, nowMs: number): boolean {
  if (a.startAt) {
    const s = Date.parse(a.startAt);
    if (!Number.isNaN(s) && nowMs < s) return false;
  }
  if (a.endAt) {
    const e = Date.parse(a.endAt);
    if (!Number.isNaN(e) && nowMs >= e) return false;
  }
  return true;
}

/**
 * 过滤有效 + 排序。
 * 排序规则与服务端一致：置顶（pinned）优先 → sortOrder 升序（越小越靠前，置顶用小值）→ 相同再按 publishedAt 降序（最新在前）。
 * 服务端已排好序时本函数是幂等的；此处仍显式排一遍，兼容旧 host 未排序的情况（双域名竞速可能命中旧版）。
 * 有效期过滤同样作为兜底保留（旧 host 可能未做服务端过滤）。
 */
export function activeSorted(list: Announcement[], nowMs: number): Announcement[] {
  const order = (a: Announcement) =>
    typeof a.sortOrder === "number" ? a.sortOrder : Number.MAX_SAFE_INTEGER;
  return list
    .filter((a) => isActive(a, nowMs))
    .slice()
    .sort((a, b) => {
      // 置顶优先：pinned 的排在前
      const p = Number(b.pinned ?? false) - Number(a.pinned ?? false);
      if (p !== 0) return p;
      const d = order(a) - order(b);
      if (d !== 0) return d;
      return Date.parse(b.publishedAt) - Date.parse(a.publishedAt);
    });
}

const READ_KEY = "announcements_read_ids";

/** 读已读 id 集合，坏数据容错为空集 */
export function loadReadIds(): Set<string> {
  try {
    const raw = localStorage.getItem(READ_KEY);
    if (!raw) return new Set();
    const arr = JSON.parse(raw);
    return Array.isArray(arr) ? new Set(arr.map(String)) : new Set();
  } catch {
    return new Set();
  }
}

export function saveReadIds(ids: Set<string>): void {
  localStorage.setItem(READ_KEY, JSON.stringify([...ids]));
}
