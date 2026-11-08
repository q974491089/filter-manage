import { useState } from "react";
import { Icon } from "./Icon";
import type { AuthSession } from "../lib/auth";

interface AccountMenuProps {
  session: AuthSession | null;
  onOpenLogin: () => void;
  onOpenMyInfo: () => void;
  onOpenSettings: () => void;
  onLogout: () => void;
}

const menuItemClass =
  "w-full flex items-center gap-sm px-md py-sm text-label-lg text-on-surface hover:bg-surface-variant/60 transition-colors text-left";

const menuIconClass = "text-[18px] text-on-surface-variant";

/**
 * 顶栏头像 + 下拉菜单（复用 AnnouncementBell 的面板模式：透明遮罩点击关闭）。
 *
 * - 未登录：菜单 = 登录 / 设置
 * - 已登录：信息头（昵称 + 邮箱）+ 我的信息 / 设置 / 退出登录
 *
 * 头像本身：未登录 = 灰圆人形（antd 默认头像的等价视觉）；已登录 = 品牌色
 * 圆底 + 昵称首字母。
 */
export default function AccountMenu({
  session,
  onOpenLogin,
  onOpenMyInfo,
  onOpenSettings,
  onLogout,
}: AccountMenuProps) {
  const [open, setOpen] = useState(false);
  const close = () => setOpen(false);

  const displayName = session?.account.displayName?.trim() || session?.account.email || "已登录";
  const initial = displayName.charAt(0).toUpperCase();

  return (
    <div className="relative">
      <button
        data-name="account-button"
        onClick={() => setOpen(!open)}
        className={
          session
            ? "w-8 h-8 rounded-full flex items-center justify-center bg-primary/10 text-primary hover:bg-primary/20 transition-colors"
            : "w-8 h-8 rounded-full flex items-center justify-center bg-surface-variant/60 text-on-surface-variant hover:bg-surface-variant hover:text-on-surface transition-colors"
        }
        title={session ? `已登录：${displayName}` : "未登录"}
        aria-label="账号菜单"
        aria-haspopup="menu"
        aria-expanded={open}
      >
        {session ? (
          <span className="font-label-md text-label-md font-medium">{initial}</span>
        ) : (
          <Icon name="person" className="text-[20px]" />
        )}
      </button>

      {/* 点击面板外任意处关闭（同 AnnouncementBell） */}
      {open ? (
        <div className="fixed inset-0 z-[150]" onClick={close} aria-hidden />
      ) : null}

      {open ? (
        <div
          role="menu"
          className="absolute right-0 top-[calc(100%+8px)] z-[160] w-[240px] bg-surface-container/95 backdrop-blur-xl rounded-xl border border-outline-variant/20 shadow-2xl overflow-hidden animate-[slide-fade-up_0.18s_cubic-bezier(0.16,1,0.3,1)_forwards] motion-reduce:animate-none"
        >
          {/* 小箭头 */}
          <div
            aria-hidden
            className="absolute -top-[5px] right-3 z-[1] w-2.5 h-2.5 rotate-45 bg-surface-container/95 border-l border-t border-outline-variant/20"
          />

          {session ? (
            <>
              {/* 信息头 */}
              <div className="px-md py-sm border-b border-outline-variant/15">
                <p className="font-label-lg text-label-lg text-on-surface truncate">
                  {session.account.displayName || "已登录"}
                </p>
                {session.account.email ? (
                  <p className="text-label-md text-label-md text-on-surface-variant truncate mt-xs">
                    {session.account.email}
                  </p>
                ) : null}
              </div>
              <div className="py-xs">
                <button role="menuitem" className={menuItemClass} onClick={() => { close(); onOpenMyInfo(); }}>
                  <Icon name="person" className={menuIconClass} />
                  我的信息
                </button>
                <button role="menuitem" className={menuItemClass} onClick={() => { close(); onOpenSettings(); }}>
                  <Icon name="settings" className={menuIconClass} />
                  设置
                </button>
                <button role="menuitem" className={menuItemClass} onClick={() => { close(); onLogout(); }}>
                  <Icon name="logout" className={menuIconClass} />
                  退出登录
                </button>
              </div>
            </>
          ) : (
            <div className="py-xs">
              <button role="menuitem" className={menuItemClass} onClick={() => { close(); onOpenLogin(); }}>
                <Icon name="person" className={menuIconClass} />
                登录
              </button>
              <button role="menuitem" className={menuItemClass} onClick={() => { close(); onOpenSettings(); }}>
                <Icon name="settings" className={menuIconClass} />
                设置
              </button>
            </div>
          )}
        </div>
      ) : null}
    </div>
  );
}
