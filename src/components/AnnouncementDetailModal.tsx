import { useEffect } from "react";
import Markdown from "react-markdown";
import { type Announcement, fmtDate } from "../lib/announcements";
import { Icon } from "./Icon";

/** 放大预览单条公告的完整内容（居中弹窗）。点击列表行打开，已在 hook 中同步标记已读。 */
export default function AnnouncementDetailModal({
  item,
  onClose,
}: {
  item: Announcement | null;
  onClose: () => void;
}) {
  // Esc 关闭
  useEffect(() => {
    if (!item) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [item, onClose]);

  if (!item) return null;

  return (
    <div
      data-component="AnnouncementDetailModal"
      data-name="detail-overlay"
      className="fixed inset-0 z-[210] flex items-center justify-center bg-black/40 backdrop-blur-sm"
      onClick={onClose}
    >
      <div
        data-component="AnnouncementDetailModal"
        data-name="detail-card"
        className="bg-surface-container/90 backdrop-blur-md border border-outline-variant/20 rounded-xl p-xl shadow-2xl shadow-black/40 w-[600px] max-w-[90vw] max-h-[85vh] flex flex-col"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-start gap-sm mb-lg">
          <div className="flex-1 min-w-0 flex items-baseline flex-wrap gap-sm">
            <h2 className="font-headline-sm text-headline-sm text-on-surface break-words">
              {item.title}
            </h2>
            {item.pinned && (
              <span className="inline-flex items-center h-4 px-1.5 rounded bg-primary/20 text-primary font-label-sm text-[10px] leading-none shrink-0">
                置顶
              </span>
            )}
            {item.level === "important" && (
              <span className="inline-flex items-center h-4 px-1.5 rounded bg-error/20 text-error font-label-sm text-[10px] leading-none shrink-0">
                重要
              </span>
            )}
          </div>
          <button
            data-name="detail-close"
            onClick={onClose}
            className="w-8 h-8 -mt-1 -mr-1 rounded-full flex items-center justify-center text-on-surface-variant hover:bg-surface-variant/40 transition-colors shrink-0"
            title="关闭"
          >
            <Icon name="close" className="text-[20px]" />
          </button>
        </div>

        <div className="font-label-sm text-label-sm text-on-surface-variant/60 mb-md">
          {fmtDate(item.publishedAt)}
        </div>

        <div className="flex-1 min-h-0 overflow-y-auto">
          <div className="prose prose-invert prose-sm max-w-none text-on-surface-variant">
            <Markdown>{item.body}</Markdown>
          </div>
        </div>

        <div className="flex justify-end mt-lg">
          <button
            data-name="detail-dismiss"
            onClick={onClose}
            className="px-lg py-sm rounded-lg bg-primary text-on-primary font-label-md text-label-md font-medium hover:opacity-90 active:scale-[0.98] transition-all"
          >
            关闭
          </button>
        </div>
      </div>
    </div>
  );
}
