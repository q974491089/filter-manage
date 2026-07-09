// 公告纯逻辑：类型 + 有效期过滤 + 排序 + 已读 localStorage 读写。
// 无 React 依赖，便于将来单测。字段与 src-tauri/src/announcements.rs 一一对应。

export type AnnouncementLevel = "normal" | "important";

export interface Announcement {
  id: string;
  title: string;
  body: string;                 // Markdown
  level: AnnouncementLevel;
  publishedAt: string;          // ISO8601 UTC
  startAt?: string | null;      // 可选，缺省=立即生效
  endAt?: string | null;        // 可选，缺省=永不过期
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

/** 过滤有效 + 按 publishedAt 倒序（最新在前） */
export function activeSorted(list: Announcement[], nowMs: number): Announcement[] {
  return list
    .filter((a) => isActive(a, nowMs))
    .slice()
    .sort((a, b) => Date.parse(b.publishedAt) - Date.parse(a.publishedAt));
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
