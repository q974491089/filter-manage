import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { serverEnv } from "../lib/serverEnv";
import {
  authErrorMessage,
  emailLogin,
  emailRegister,
  exchangeTicket,
  getAuthProviders,
  githubStart,
  isUnauthorizedError,
  sendEmailCode,
  type AuthProvider,
  type AuthSession,
} from "../lib/auth";
import { Icon } from "./Icon";
import { useModalAccessibility } from "../hooks/useModalAccessibility";

interface AuthModalProps {
  open: boolean;
  onClose: () => void;
  showToast: (type: "success" | "error", text: string) => void;
  /** 登录/注册成功（已拿到 Session 并存好令牌）：App 更新会话状态 */
  onLoggedIn: (session: AuthSession) => void;
}

type Tab = "email" | "github";
type EmailMode = "register" | "login";

const inputClass =
  "w-full h-8 px-sm rounded-md bg-surface-container-high border border-outline-variant/25 outline-none focus:border-primary/60 focus-visible:ring-2 focus-visible:ring-primary/30 text-body-sm text-on-surface transition-colors";
const primaryBtnClass =
  "w-full h-9 rounded-md bg-primary text-on-primary text-label-lg font-medium hover:bg-primary/90 disabled:opacity-50 disabled:pointer-events-none transition-colors";
const tabBtnClass = (active: boolean) =>
  `flex-1 h-8 rounded-md text-label-lg font-medium transition-colors ${
    active
      ? "bg-primary/10 text-primary"
      : "text-on-surface-variant hover:bg-surface-variant/50 hover:text-on-surface"
  }`;

const RESEND_SECONDS = 60;

/** 未选任何登录方式时的默认路径：邮箱注册。 */
export default function AuthModal({ open, onClose, showToast, onLoggedIn }: AuthModalProps) {
  const [tab, setTab] = useState<Tab>("email");
  const [emailMode, setEmailMode] = useState<EmailMode>("register");
  const [email, setEmail] = useState("");
  const [code, setCode] = useState("");
  const [password, setPassword] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [providers, setProviders] = useState<AuthProvider[] | null>(null);
  const [cooldown, setCooldown] = useState(0);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState("");
  const modalRef = useModalAccessibility(open, onClose);
  const cooldownRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const isRegister = emailMode === "register";
  const githubProvider = providers?.find((p) => p.id === "github") ?? null;

  // 打开时重置表单并拉取可用登录方式（决定 GitHub 按钮显隐）。
  useEffect(() => {
    if (!open) return;
    setTab("email");
    setEmailMode("register");
    setEmail("");
    setCode("");
    setPassword("");
    setDisplayName("");
    setError("");
    setSubmitting(false);
    setProviders(null);
    getAuthProviders()
      .then(setProviders)
      .catch((err) => setError(authErrorMessage(err)));
  }, [open]);

  // 发送按钮倒计时；弹窗关闭时清理。
  useEffect(() => {
    if (!open) {
      if (cooldownRef.current) clearInterval(cooldownRef.current);
      cooldownRef.current = null;
      setCooldown(0);
    }
  }, [open]);

  const startCooldown = useCallback((seconds: number) => {
    setCooldown(seconds);
    if (cooldownRef.current) clearInterval(cooldownRef.current);
    cooldownRef.current = setInterval(() => {
      setCooldown((s) => {
        if (s <= 1) {
          if (cooldownRef.current) clearInterval(cooldownRef.current);
          cooldownRef.current = null;
          return 0;
        }
        return s - 1;
      });
    }, 1000);
  }, []);

  /** GitHub 授权：起本地监听 → 开系统浏览器 → 等 auth://ticket 事件换会话。 */
  const startGithub = useCallback(async () => {
    setError("");
    setSubmitting(true);
    let unlisten: (() => void) | null = null;
    try {
      const port = await githubStart();
      unlisten = await listen<string>("auth://ticket", (event) => {
        void (async () => {
          unlisten?.();
          unlisten = null;
          try {
            const session = await exchangeTicket(event.payload);
            onLoggedIn(session);
            showToast("success", "登录成功");
          } catch (err) {
            setError(authErrorMessage(err));
          } finally {
            setSubmitting(false);
          }
        })();
      });
      await openUrl(`${serverEnv().marketApiBase}/api/market/auth/github/start?port=${port}`);
      showToast("success", "已在浏览器打开 GitHub 授权页面");
    } catch (err) {
      unlisten?.();
      setSubmitting(false);
      setError(authErrorMessage(err));
    }
  }, [onLoggedIn, showToast]);

  const handleSendCode = useCallback(async () => {
    if (!email.trim() || cooldown > 0) return;
    setError("");
    try {
      const result = await sendEmailCode(email.trim());
      showToast("success", "验证码已发送，请查收邮箱（注意垃圾邮件）");
      startCooldown(Math.max(RESEND_SECONDS, Math.ceil((result.expiresIn || 600) / 10)));
    } catch (err) {
      setError(authErrorMessage(err));
    }
  }, [email, cooldown, showToast, startCooldown]);

  const handleSubmit = useCallback(async () => {
    if (submitting) return;
    setError("");
    setSubmitting(true);
    try {
      const session = isRegister
        ? await emailRegister(email.trim(), password, displayName.trim(), code.trim())
        : await emailLogin(email.trim(), password);
      onLoggedIn(session);
      showToast("success", isRegister ? "注册成功，已登录" : "登录成功");
    } catch (err) {
      setError(authErrorMessage(err));
    } finally {
      setSubmitting(false);
    }
  }, [submitting, isRegister, email, password, displayName, code, onLoggedIn, showToast]);

  // 会话在别处失效（401）时清掉错误并回表单态——父组件会把 open 置回 true。
  useEffect(() => {
    if (open && isUnauthorizedError(error)) setError("");
  }, [open, error]);

  if (!open) return null;

  const canSubmit =
    !submitting &&
    email.trim() !== "" &&
    password !== "" &&
    (!isRegister || code.trim() !== "");

  return (
    <div className="fixed inset-0 z-[300] flex items-center justify-center bg-black/60 p-3 backdrop-blur-[2px] sm:p-5">
      <div
        ref={modalRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="auth-modal-title"
        tabIndex={-1}
        className="relative bg-surface-container rounded-xl shadow-2xl border border-outline-variant/25 w-[420px] max-w-[94vw] max-h-[90vh] flex flex-col overflow-hidden animate-[slide-fade-up_0.28s_cubic-bezier(0.16,1,0.3,1)_forwards] motion-reduce:animate-none"
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
            <h2 id="auth-modal-title" className="font-headline-sm text-headline-sm text-on-surface">
              登录 Filter Manage
            </h2>
            <p className="mt-xs text-body-sm text-on-surface-variant">
              登录后方案投稿与分享归到一个账号下
            </p>
          </div>
          <button
            onClick={onClose}
            aria-label="关闭"
            className="p-xs rounded-md text-on-surface-variant hover:bg-surface-variant/60 hover:text-on-surface transition-colors"
          >
            <Icon name="close" className="text-[20px]" />
          </button>
        </div>

        {/* Tabs：邮箱是默认路径 */}
        <div className="relative px-lg pt-md flex gap-xs shrink-0">
          <button type="button" className={tabBtnClass(tab === "email")} onClick={() => setTab("email")}>
            邮箱
          </button>
          {githubProvider ? (
            <button type="button" className={tabBtnClass(tab === "github")} onClick={() => setTab("github")}>
              GitHub
            </button>
          ) : null}
        </div>

        {/* Body */}
        <div className="relative px-lg pb-lg pt-md overflow-y-auto custom-scrollbar">
          {tab === "email" ? (
            <form
              className="flex flex-col gap-sm"
              onSubmit={(e) => {
                e.preventDefault();
                void handleSubmit();
              }}
            >
              <label className="flex flex-col gap-xs">
                <span className="text-label-md text-on-surface-variant">邮箱</span>
                <input
                  className={inputClass}
                  type="email"
                  autoComplete="email"
                  placeholder="支持 QQ 邮箱、Gmail"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  disabled={submitting}
                />
              </label>

              {isRegister ? (
                <label className="flex flex-col gap-xs">
                  <span className="text-label-md text-on-surface-variant">验证码</span>
                  <div className="flex gap-xs">
                    <input
                      className={inputClass}
                      inputMode="numeric"
                      maxLength={6}
                      placeholder="6 位数字"
                      value={code}
                      onChange={(e) => setCode(e.target.value.replace(/\D/g, ""))}
                      disabled={submitting}
                    />
                    <button
                      type="button"
                      onClick={() => void handleSendCode()}
                      disabled={cooldown > 0 || submitting || email.trim() === ""}
                      className="shrink-0 h-8 px-sm rounded-md border border-outline-variant/45 text-label-md text-on-surface-variant hover:bg-surface-variant/50 hover:text-on-surface disabled:opacity-50 disabled:pointer-events-none transition-colors"
                    >
                      {cooldown > 0 ? `${cooldown}s 后重发` : "发送验证码"}
                    </button>
                  </div>
                </label>
              ) : null}

              <label className="flex flex-col gap-xs">
                <span className="text-label-md text-on-surface-variant">密码</span>
                <input
                  className={inputClass}
                  type="password"
                  autoComplete={isRegister ? "new-password" : "current-password"}
                  placeholder={isRegister ? "至少 8 个字符" : ""}
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  disabled={submitting}
                />
              </label>

              {isRegister ? (
                <label className="flex flex-col gap-xs">
                  <span className="text-label-md text-on-surface-variant">昵称（可选）</span>
                  <input
                    className={inputClass}
                    type="text"
                    maxLength={50}
                    placeholder="投稿时展示的名字"
                    value={displayName}
                    onChange={(e) => setDisplayName(e.target.value)}
                    disabled={submitting}
                  />
                </label>
              ) : null}

              {error ? <p className="text-body-sm text-error">{error}</p> : null}

              <button type="submit" className={primaryBtnClass} disabled={!canSubmit}>
                {submitting ? "请稍候…" : isRegister ? "注册并登录" : "登录"}
              </button>

              <button
                type="button"
                onClick={() => {
                  setEmailMode(isRegister ? "login" : "register");
                  setError("");
                  setCode("");
                }}
                className="mt-xs text-label-md text-on-surface-variant hover:text-primary transition-colors"
              >
                {isRegister ? "已有账号？去登录" : "没有账号？注册一个"}
              </button>
            </form>
          ) : (
            <div className="flex flex-col gap-sm">
              <p className="text-body-sm text-on-surface-variant">
                点击下方按钮会在浏览器打开 GitHub 授权页面，授权完成后自动回到客户端完成登录。
              </p>
              {error ? <p className="text-body-sm text-error">{error}</p> : null}
              <button
                type="button"
                onClick={() => void startGithub()}
                disabled={submitting}
                className={primaryBtnClass}
              >
                {submitting ? "等待授权…" : "通过 GitHub 登录"}
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
