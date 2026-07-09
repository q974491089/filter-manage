import Markdown from "react-markdown";
import type { Announcement } from "../lib/announcements";
import { Icon } from "./Icon";

function fmtDate(iso: string): string {
  const t = Date.parse(iso);
  if (Number.isNaN(t)) return "";
  return new Date(t).toLocaleDateString();
}

export default function AnnouncementModal({
  items,
  onDismiss,
}: {
  items: Announcement[];
  onDismiss: () => void;
}) {
  if (items.length === 0) return null;

  return (
    <div
      data-component="AnnouncementModal"
      data-name="announcement-overlay"
      className="fixed inset-0 z-[200] flex items-center justify-center bg-black/40 backdrop-blur-sm"
    >
      <div
        data-component="AnnouncementModal"
        data-name="announcement-card"
        className="bg-surface-container/80 backdrop-blur-md border border-outline-variant/20 rounded-xl p-xl shadow-2xl shadow-black/40 w-[600px] max-h-[85vh] flex flex-col"
      >
        <div className="flex items-center gap-md mb-lg">
          <Icon name="notifications" filled className="text-[24px] text-primary" />
          <h2 className="font-headline-sm text-headline-sm text-on-surface">重要公告</h2>
          <div className="flex-1" />
        </div>

        <div className="flex-1 min-h-0 overflow-y-auto flex flex-col gap-lg">
          {items.map((a) => (
            <div key={a.id} data-component="AnnouncementModal" data-name="announcement-item">
              <div className="flex items-baseline gap-sm mb-sm">
                <h3 className="font-title-sm text-title-sm text-on-surface">{a.title}</h3>
                <span className="font-label-sm text-label-sm text-on-surface-variant/60">
                  {fmtDate(a.publishedAt)}
                </span>
              </div>
              <div className="bg-surface-variant/30 rounded-lg p-md">
                <div className="prose prose-invert prose-sm max-w-none text-on-surface-variant">
                  <Markdown>{a.body}</Markdown>
                </div>
              </div>
            </div>
          ))}
        </div>

        <div className="flex justify-end mt-lg">
          <button
            data-component="AnnouncementModal"
            data-name="announcement-dismiss"
            onClick={onDismiss}
            className="px-lg py-sm rounded-lg bg-primary text-on-primary font-label-md text-label-md font-medium hover:opacity-90 active:scale-[0.98] transition-all"
          >
            我知道了
          </button>
        </div>
      </div>
    </div>
  );
}
