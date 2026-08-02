import { useEffect, useState, useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  type Announcement,
  type AnnouncementCategory,
  activeSorted,
  normalizeCategory,
  loadReadIds,
  saveReadIds,
} from "../lib/announcements";

export function useAnnouncements() {
  const [items, setItems] = useState<Announcement[]>([]);
  const [readIds, setReadIds] = useState<Set<string>>(() => loadReadIds());
  const [panelOpen, setPanelOpen] = useState(false);
  const [modalItems, setModalItems] = useState<Announcement[]>([]);
  const [detailItem, setDetailItem] = useState<Announcement | null>(null);
  const [activeTab, setActiveTab] = useState<AnnouncementCategory>("announcement");

  // 启动拉取（随 check_update 一并完成；失败静默）
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const raw = await invoke<Announcement[]>("get_announcements");
        if (cancelled) return;
        const active = activeSorted(raw, Date.now());
        setItems(active);
        // 默认停在「公告」；若公告为空而通知有内容，开局就切到通知，避免打开面板是空的
        if (
          !active.some((a) => normalizeCategory(a) === "announcement") &&
          active.some((a) => normalizeCategory(a) === "notification")
        ) {
          setActiveTab("notification");
        }
        const read = loadReadIds();
        const importantUnread = active.filter(
          (a) => a.level === "important" && !read.has(a.id)
        );
        if (importantUnread.length > 0) setModalItems(importantUnread);
      } catch (e) {
        console.error("[Announcements] fetch failed:", e);
        // 静默：无公告即可，不打扰用户
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const isRead = useCallback((id: string) => readIds.has(id), [readIds]);

  // 按分类拆桶，保留 activeSorted 已排好的顺序（置顶 → sortOrder → 时间倒序）
  const byCategory = useMemo(() => {
    const buckets: Record<AnnouncementCategory, Announcement[]> = {
      announcement: [],
      notification: [],
    };
    items.forEach((a) => buckets[normalizeCategory(a)].push(a));
    return buckets;
  }, [items]);

  /** 当前 Tab 要渲染的列表 */
  const visibleItems = byCategory[activeTab];

  /** 各 Tab 未读数，用于 Tab 标签上的徽标 */
  const unreadByCategory = useMemo(
    () => ({
      announcement: byCategory.announcement.filter((a) => !readIds.has(a.id)).length,
      notification: byCategory.notification.filter((a) => !readIds.has(a.id)).length,
    }),
    [byCategory, readIds]
  );

  // 铃铛徽标：两类总未读数
  const unreadCount = items.reduce(
    (n, a) => (readIds.has(a.id) ? n : n + 1),
    0
  );

  const markRead = useCallback((id: string) => {
    setReadIds((prev) => {
      if (prev.has(id)) return prev;
      const next = new Set(prev);
      next.add(id);
      saveReadIds(next);
      return next;
    });
  }, []);

  const markAllRead = useCallback(() => {
    setReadIds((prev) => {
      const next = new Set(prev);
      items.forEach((a) => next.add(a.id));
      saveReadIds(next);
      return next;
    });
  }, [items]);

  // 放大预览某条公告：打开详情并标记已读
  const openDetail = useCallback(
    (a: Announcement) => {
      setDetailItem(a);
      markRead(a.id);
    },
    [markRead]
  );

  const closeDetail = useCallback(() => setDetailItem(null), []);

  // 关闭重要公告弹窗：把弹窗内的公告标记已读
  const dismissModal = useCallback(() => {
    setModalItems((cur) => {
      setReadIds((prev) => {
        const next = new Set(prev);
        cur.forEach((a) => next.add(a.id));
        saveReadIds(next);
        return next;
      });
      return [];
    });
  }, []);

  return {
    items,
    visibleItems,
    unreadCount,
    unreadByCategory,
    activeTab,
    setActiveTab,
    isRead,
    panelOpen,
    setPanelOpen,
    markRead,
    markAllRead,
    modalItems,
    dismissModal,
    detailItem,
    openDetail,
    closeDetail,
  };
}
