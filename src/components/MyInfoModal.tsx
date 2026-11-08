import { Icon } from "./Icon";
import { useModalAccessibility } from "../hooks/useModalAccessibility";
import type { AuthSession } from "../lib/auth";

interface MyInfoModalProps {
  open: boolean;
  onClose: () => void;
  session: AuthSession;
  /** 预留：以后加「我的投稿」入口 */
  onOpenSettings: () => void;
}

const rowClass = "flex items-center justify-between gap-md py-sm border-b border-outline-variant/10 last:border-b-0";

/** 我的信息：账号资料只读展示（大头像 + 昵称/邮箱/注册时间）。 */
export default function MyInfoModal({ open, onClose, session, onOpenSettings }: MyInfoModalProps) {
  const modalRef = useModalAccessibility(open, onClose);
  if (!open) return null;

  const { account } = session;
  const displayName = account.displayName?.trim() || account.email || "已登录";
  const initial = displayName.charAt(0).toUpperCase();
  const createdAt = (() => {
    try {
      return new Date(account.createdAt).toLocaleDateString("zh-CN", {
        year: "numeric",
        month: "long",
        day: "numeric",
      });
    } catch {
      return account.createdAt;
    }
  })();

  return (
    <div className="fixed inset-0 z-[300] flex items-center justify-center bg-black/60 p-3 backdrop-blur-[2px] sm:p-5">
      <div
        ref={modalRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="my-info-title"
        tabIndex={-1}
        className="relative bg-surface-container rounded-xl shadow-2xl border border-outline-variant/25 w-[420px] max-w-[94vw] flex flex-col overflow-hidden animate-[slide-fade-up_0.28s_cubic-bezier(0.16,1,0.3,1)_forwards] motion-reduce:animate-none"
      >
        <div
          aria-hidden
          className="pointer-events-none absolute inset-x-0 top-0 h-28"
          style={{
            backgroundImage:
              "radial-gradient(120% 120% at 50% 0%, rgb(var(--color-primary) / 0.07), transparent 62%)",
          }}
        />
        {/* Header */}
        <div className="relative flex items-start justify-between gap-md px-lg pt-lg pb-md shrink-0 border-b border-outline-variant/15">
          <div>
            <h2 id="my-info-title" className="font-headline-sm text-headline-sm text-on-surface">
              我的信息
            </h2>
            <p className="mt-xs text-body-sm text-on-surface-variant">账号资料（只读）</p>
          </div>
          <button
            onClick={onClose}
            aria-label="关闭"
            className="p-xs rounded-md text-on-surface-variant hover:bg-surface-variant/60 hover:text-on-surface transition-colors"
          >
            <Icon name="close" className="text-[20px]" />
          </button>
        </div>

        {/* Body */}
        <div className="relative px-lg pb-lg pt-md flex flex-col gap-md">
          <div className="flex items-center gap-md">
            <div className="w-14 h-14 rounded-full bg-primary/10 text-primary flex items-center justify-center shrink-0">
              <span className="font-headline-md text-headline-md font-medium">{initial}</span>
            </div>
            <div className="min-w-0">
              <p className="font-title-sm text-title-sm text-on-surface truncate">{displayName}</p>
              {account.email ? (
                <p className="text-body-sm text-body-sm text-on-surface-variant truncate">{account.email}</p>
              ) : null}
            </div>
          </div>

          <div className="rounded-lg bg-surface-container-high/60 px-md py-xs">
            <div className={rowClass}>
              <span className="text-label-md text-label-md text-on-surface-variant">邮箱</span>
              <span className="text-label-lg text-label-lg text-on-surface truncate">
                {account.email || "—"}
              </span>
            </div>
            <div className={rowClass}>
              <span className="text-label-md text-label-md text-on-surface-variant">注册时间</span>
              <span className="text-label-lg text-label-lg text-on-surface">{createdAt}</span>
            </div>
            <div className={rowClass}>
              <span className="text-label-md text-label-md text-on-surface-variant">状态</span>
              <span className="text-label-lg text-label-lg text-on-surface">
                {account.status === "ACTIVE" ? "正常" : account.status}
              </span>
            </div>
          </div>

          <button
            onClick={() => {
              onClose();
              onOpenSettings();
            }}
            className="w-full h-9 rounded-md border border-outline-variant/45 text-label-lg text-on-surface-variant hover:bg-surface-variant/50 hover:text-on-surface transition-colors"
          >
            前往设置
          </button>
        </div>
      </div>
    </div>
  );
}
