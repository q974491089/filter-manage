import { type Announcement, fmtDate, plainPreview } from "../lib/announcements";
import { Icon } from "./Icon";
import AnnouncementDetailModal from "./AnnouncementDetailModal";

export default function AnnouncementBell({
  items,
  unreadCount,
  isRead,
  open,
  setOpen,
  markAllRead,
  detailItem,
  openDetail,
  closeDetail,
}: {
  items: Announcement[];
  unreadCount: number;
  isRead: (id: string) => boolean;
  open: boolean;
  setOpen: (v: boolean) => void;
  markAllRead: () => void;
  detailItem: Announcement | null;
  openDetail: (a: Announcement) => void;
  closeDetail: () => void;
}) {
  return (
    <div data-component="AnnouncementBell" data-name="bell-wrapper" className="relative">
      <button
        data-component="AnnouncementBell"
        data-name="bell-button"
        onClick={() => setOpen(!open)}
        className="relative w-8 h-8 rounded-full flex items-center justify-center text-on-surface-variant hover:bg-primary/10 transition-colors"
        title="公告（通知）"
      >
        <Icon name="notifications" className="text-[20px]" />
        {unreadCount > 0 && (
          <span
            data-name="unread-badge"
            className="absolute -top-0.5 -right-0.5 min-w-[16px] h-4 px-1 rounded-full bg-error text-on-error text-[10px] leading-4 font-bold flex items-center justify-center"
          >
            {unreadCount > 9 ? "9+" : unreadCount}
          </span>
        )}
      </button>

      {open && (
        <>
          {/* click-outside backdrop */}
          <div
            data-name="bell-backdrop"
            className="fixed inset-0 z-[150]"
            onClick={() => setOpen(false)}
          />
          <div
            data-component="AnnouncementBell"
            data-name="bell-panel"
            className="absolute right-0 top-[calc(100%+8px)] z-[160] w-[380px]"
          >
            {/* 指向铃铛图标的小三角：绘制在卡片之上并向下叠 1px，盖住卡片顶边，使尖角与矩形连成一体 */}
            <span
              data-name="bell-caret"
              aria-hidden
              className="absolute -top-[5px] right-3 z-[1] w-2.5 h-2.5 rotate-45 bg-surface-container/95 border-l border-t border-outline-variant/20"
            />

            <div
              data-name="bell-panel-card"
              className="relative max-h-[70vh] flex flex-col bg-surface-container/95 backdrop-blur-md border border-outline-variant/20 rounded-xl shadow-2xl shadow-black/40 overflow-hidden"
            >
            <div className="flex items-center justify-between px-md py-sm border-b border-outline-variant/20">
              <span className="font-title-sm text-title-sm text-on-surface">公告（通知）</span>
              {unreadCount > 0 && (
                <button
                  data-name="mark-all-read"
                  onClick={markAllRead}
                  className="font-label-sm text-label-sm text-on-surface-variant hover:text-primary transition-colors"
                >
                  全部已读
                </button>
              )}
            </div>

            <div className="flex-1 overflow-y-auto">
              {items.length === 0 ? (
                <div data-name="bell-empty" className="px-md py-lg text-center font-label-md text-label-md text-on-surface-variant/60">
                  暂无公告
                </div>
              ) : (
                items.map((a) => {
                  const unread = !isRead(a.id);
                  const preview = plainPreview(a.body);
                  return (
                    <button
                      key={a.id}
                      data-component="AnnouncementBell"
                      data-name="announcement-row"
                      onClick={() => openDetail(a)}
                      title="点击查看完整公告"
                      className="w-full text-left px-md py-sm border-b border-outline-variant/30 hover:bg-surface-variant/30 transition-colors"
                    >
                      {/* 标题行：单行截断 */}
                      <div className="flex items-center gap-sm mb-xs">
                        {unread && <span className="w-2 h-2 rounded-full bg-primary shrink-0" />}
                        {a.pinned && (
                          <span className="inline-flex items-center h-4 px-1.5 rounded bg-primary/20 text-primary font-label-sm text-[10px] leading-none shrink-0">置顶</span>
                        )}
                        {a.level === "important" && (
                          <span className="inline-flex items-center h-4 px-1.5 rounded bg-error/20 text-error font-label-sm text-[10px] leading-none shrink-0">重要</span>
                        )}
                        <span className="font-title-sm text-title-sm text-on-surface truncate">{a.title}</span>
                        <span className="flex-1" />
                        <span className="font-label-sm text-label-sm text-on-surface-variant/60 shrink-0">{fmtDate(a.publishedAt)}</span>
                      </div>
                      {/* 描述：最多两行，超出省略 */}
                      {preview && (
                        <p className="font-body-sm text-body-sm text-on-surface-variant/80 line-clamp-2">
                          {preview}
                        </p>
                      )}
                    </button>
                  );
                })
              )}
            </div>
            </div>
          </div>
        </>
      )}

      {/* 放大预览：完整公告内容 */}
      <AnnouncementDetailModal item={detailItem} onClose={closeDetail} />
    </div>
  );
}
