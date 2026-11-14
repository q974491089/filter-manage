import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "../lib/ipc";
import { debugNote } from "../lib/debug";
import BuiltinPreviewPicker from "./BuiltinPreviewPicker";
import { DEFAULT_PREVIEW_SCENE, type BuiltinSceneId } from "../lib/builtinPreview";
import { sha256Hex, getContributorId, getSubmissionTokens, uploadToGateway } from "../lib/submissions";
import { checkShareIcc, ContentRejectedError, createShare, shareUrl, type ShareColor } from "../lib/share";
import { Icon } from "./Icon";
import { useModalAccessibility } from "../hooks/useModalAccessibility";

interface ColorConfig {
  name: string;
  brightness: number;
  contrast: number;
  gamma: number;
  digital_vibrance: number;
  rgb_r?: number;
  rgb_g?: number;
  rgb_b?: number;
  resolution?: string | null;
  icc_profile: string | null;
}

interface SharePlanModalProps {
  open: boolean;
  /** 要分享的方案；null 表示当前未保存的编辑状态 */
  config: ColorConfig | null;
  onClose: () => void;
  showToast: (type: "success" | "error", text: string) => void;
  /** 分享创建成功（链接已进剪贴板）：App 把 slug 记入会话已处理集合，
   *  防止剪贴板检测把自己的分享链接再弹成「导入」 */
  onShared?: (slug: string) => void;
}

type IccState =
  | "none" // 方案本来就不含 ICC
  | "checking" // 读取/哈希/询问服务端
  | "in-library" // 已在库，可直接创建
  | "uploading" // 自动上传中
  | "failed" // 自动上传失败：重试 或 放弃 ICC 仅分享参数
  | "params-only"; // 无法附带 ICC（本机没有/文件异常/用户跳过），iccNote 说明原因

/** 客户端「未选择 ICC」的哨兵值：方案保存时会把当前选中的 profile 名写进去 */
const NO_ICC_SENTINELS = new Set(["Default", "Default.icc"]);

const inputClass =
  "w-full h-8 px-sm rounded-md bg-surface-container-high border border-outline-variant/25 outline-none focus:border-primary/60 focus-visible:ring-2 focus-visible:ring-primary/30 text-body-sm text-on-surface transition-colors";
const textareaClass =
  "w-full px-sm py-sm rounded-md bg-surface-container-high border border-outline-variant/25 outline-none focus:border-primary/60 focus-visible:ring-2 focus-visible:ring-primary/30 text-body-sm text-on-surface transition-colors resize-none";

export default function SharePlanModal({
  open,
  config,
  onClose,
  showToast,
  onShared,
}: SharePlanModalProps) {
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [nickname, setNickname] = useState("");
  const [previewScene, setPreviewScene] = useState<BuiltinSceneId>(DEFAULT_PREVIEW_SCENE);
  const [iccState, setIccState] = useState<IccState>("none");
  const [iccNote, setIccNote] = useState("");
  const [iccBytes, setIccBytes] = useState<{ base64: string; sha256: string; size: number } | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [stage, setStage] = useState("");
  const [link, setLink] = useState("");
  const [fieldError, setFieldError] = useState<{ field: string; message: string } | null>(null);
  const modalRef = useModalAccessibility(open, onClose, true, !submitting);

  const rawIccName = config?.icc_profile ?? "";
  const iccName = NO_ICC_SENTINELS.has(rawIccName) ? "" : rawIccName;
  const hasIcc = iccName !== "";

  // 打开时重置并开始检测 ICC 是否已在库里
  useEffect(() => {
    if (!open || !config) return;
    setTitle(config.name);
    setDescription("");
    setNickname("");
    setPreviewScene(DEFAULT_PREVIEW_SCENE);
    setIccState(hasIcc ? "checking" : "none");
    setIccNote("");
    setIccBytes(null);
    setLink("");
    setStage("");
    setSubmitting(false);
    setFieldError(null);
  }, [open, config, hasIcc]);

  const nicknameRef = useRef("");
  nicknameRef.current = nickname;

  /**
   * 把 ICC 字节传到内容寻址网关。稳定引用（nickname 走 ref），
   * 检测流程和手动重试共用，避免依赖变化触发重新检测。
   */
  const runIccUpload = useCallback(
    async (bytes: { base64: string; sha256: string }): Promise<boolean> => {
      setIccState("uploading");
      setStage("正在准备配置文件");
      try {
        const contributorId = await getContributorId();
        const tokens = await getSubmissionTokens(contributorId, nicknameRef.current.trim());
        const binary = atob(bytes.base64);
        const buffer = new Uint8Array(binary.length);
        for (let i = 0; i < binary.length; i += 1) buffer[i] = binary.charCodeAt(i);
        await uploadToGateway(
          `${tokens.gatewayUrl}/v1/icc`,
          tokens.iccToken,
          bytes.sha256,
          buffer.buffer,
          "application/vnd.iccprofile",
        );
        setIccState("in-library");
        debugNote("分享上传 ICC 完成");
        return true;
      } catch (err) {
        setIccNote(err instanceof Error ? err.message : String(err));
        setIccState("failed");
        return false;
      } finally {
        setStage("");
      }
    },
    [],
  );

  // ICC 检测：读本地字节 → 算 sha → 问服务端；不在库就**自动上传**，
  // 失败才停下来给用户选择（重试 / 仅分享参数）。
  useEffect(() => {
    if (!open || !hasIcc) return;
    let cancelled = false;
    void (async () => {
      try {
        setStage("正在准备配置文件");
        const file = await invoke<{ name: string; sizeBytes: number; base64: string }>(
          "read_icc_file",
          { name: iccName },
        );
        if (cancelled) return;
        const binary = atob(file.base64);
        const bytes = new Uint8Array(binary.length);
        for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
        const sha = await sha256Hex(bytes.buffer);
        if (cancelled) return;
        setIccBytes({ base64: file.base64, sha256: sha, size: file.sizeBytes });
        const exists = await checkShareIcc(sha);
        if (cancelled) return;
        if (exists) {
          setIccState("in-library");
          return;
        }
        // 库里没有：自动上传，不等用户点
        await runIccUpload({ base64: file.base64, sha256: sha });
      } catch (err) {
        if (cancelled) return;
        const msg = err instanceof Error ? err.message : String(err);
        debugNote(`分享检测 ICC 失败：${msg}`);
        // 如实展示失败原因（文件不存在 / 校验失败 / 客户端过旧），
        // 不再笼统说「本机找不到」；方案仍可只分享参数
        setIccNote(msg.includes("not found") ? `客户端版本过旧，请重启后再试（${msg}）` : msg);
        setIccState("params-only");
      } finally {
        if (!cancelled) setStage("");
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [open, hasIcc, iccName, runIccUpload]);

  const color = useMemo<ShareColor>(
    () => ({
      brightness: config?.brightness ?? 0,
      contrast: config?.contrast ?? 0,
      gamma: config?.gamma ?? 1,
      digitalVibrance: config?.digital_vibrance ?? 50,
      rgbR: config?.rgb_r ?? 0,
      rgbG: config?.rgb_g ?? 0,
      rgbB: config?.rgb_b ?? 0,
    }),
    [config],
  );

  const summaryPairs: Array<[string, string]> = [
    ["亮度", String(color.brightness)],
    ["对比度", String(color.contrast)],
    ["Gamma", String(color.gamma)],
    ["数字振动", String(color.digitalVibrance)],
    ["RGB", `${color.rgbR} / ${color.rgbG} / ${color.rgbB}`],
    ["分辨率", config?.resolution || "不调整"],
  ];

  if (!open || !config) return null;

  const iccAttached = iccState === "in-library" && iccBytes !== null;
  const busy = submitting || iccState === "checking" || iccState === "uploading";
  const canCreate = !busy && title.trim().length > 0;

  async function submit() {
    if (!canCreate) return;
    setSubmitting(true);
    try {
      const contributorId = await getContributorId();

      setStage("生成内置预览并创建分享链接");
      const result = await createShare({
        title: title.trim(),
        description: description.trim(),
        color,
        resolution: config?.resolution ?? "",
        iccSha256: iccAttached ? iccBytes!.sha256 : "",
        iccName: iccAttached ? iccName : "",
        previewScene,
        contributorId,
        contributorName: nickname.trim(),
      });
      const url = shareUrl(result.slug);
      try {
        await invoke("copy_text", { text: url });
        showToast("success", "分享链接已复制到剪贴板");
      } catch {
        showToast("success", "分享链接已生成");
      }
      debugNote(`分享方案「${title.trim()}」→ ${result.slug}`);
      // 会话内不再把自己的链接当「待导入」弹窗（剪贴板检测回声抑制）
      onShared?.(result.slug);
      setLink(url);
    } catch (err) {
      if (err instanceof ContentRejectedError) {
        const inputId = {
          title: "share-plan-title-input",
          description: "share-plan-description",
          nickname: "share-plan-nickname",
        }[err.field];
        if (!inputId) {
          showToast("error", err.message);
          return;
        }
        setFieldError({ field: err.field, message: err.message });
        requestAnimationFrame(() => document.getElementById(inputId)?.focus());
        return;
      }
      showToast("error", err instanceof Error ? err.message : String(err));
    } finally {
      setSubmitting(false);
      setStage("");
    }
  }

  const iccStatusText = (() => {
    switch (iccState) {
      case "checking":
        return "正在准备配置文件…";
      case "in-library":
        return "会随这条分享一起提供";
      case "uploading":
        return stage || "正在上传配置文件…";
      case "failed":
        return `上传失败：${iccNote}`;
      case "params-only":
        return iccNote || "这个文件无法附带，收到的方案只调整色彩参数";
      default:
        return "这条方案不含配置文件，只分享色彩参数与分辨率";
    }
  })();

  const iccTone =
    iccState === "in-library"
      ? { box: "bg-tertiary/10" }
      : iccState === "failed"
        ? { box: "bg-error/10" }
        : iccState === "uploading"
          ? { box: "bg-amber-400/10" }
          : iccState === "checking"
            ? { box: "bg-primary/10" }
            : { box: "bg-surface-container-high/50" };

  return (
    <div className="fixed inset-0 z-[300] flex items-center justify-center bg-black/60 p-3 backdrop-blur-[2px] sm:p-5">
      <div
        ref={modalRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="share-plan-title"
        tabIndex={-1}
        className="relative bg-surface-container rounded-xl shadow-2xl border border-outline-variant/25 w-[560px] max-w-[94vw] max-h-[90vh] flex flex-col overflow-hidden animate-[slide-fade-up_0.28s_cubic-bezier(0.16,1,0.3,1)_forwards] motion-reduce:animate-none"
      >
        {/* 环境景深：头部一枚极淡的品牌色径向洗染（随主题 token 走） */}
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
            <h2 id="share-plan-title" className="font-headline-sm text-headline-sm text-on-surface">
              分享方案
            </h2>
            <p className="mt-xs text-body-sm text-on-surface-variant">
              生成一条链接，别人打开后可以一键导入这套参数
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

        {/* Body */}
        <div className="relative flex-1 min-h-0 overflow-y-auto custom-scrollbar">
          {link ? (
            <div className="px-lg py-lg space-y-md">
              <div className="flex items-center gap-sm rounded-md bg-tertiary/10 px-md py-sm text-tertiary font-label-md text-label-md">
                <Icon name="check_circle" filled className="text-[18px]" />
                分享链接已生成，已复制到剪贴板
              </div>
              <div className="rounded-md border border-outline-variant/25 bg-surface-container-high px-md py-sm break-all font-mono text-[13px] leading-relaxed text-on-surface select-all">
                {link}
              </div>
              <div className="flex gap-sm">
                <button
                  onClick={() => void invoke("copy_text", { text: link })}
                  className="inline-flex items-center justify-center gap-1 min-h-10 flex-1 rounded-md border border-primary/40 px-md py-sm text-primary font-label-md text-label-md hover:bg-primary/20 transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-primary"
                >
                  <Icon name="content_copy" className="text-[15px]" />
                  复制链接
                </button>
                <button
                  onClick={onClose}
                  className="min-h-10 flex-1 rounded-md border border-outline-variant/45 px-md py-sm text-on-surface-variant font-label-md text-label-md hover:bg-surface-variant/50 transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-primary"
                >
                  完成
                </button>
              </div>
            </div>
          ) : (
            <div className="px-lg py-lg space-y-xl">
              <div>
                <p className="mb-sm font-label-md text-label-md text-on-surface-variant">
                  效果预览 <span className="text-on-surface-variant/60">· 挑一张游戏画面，接收方看到的预览就是它</span>
                </p>
                <BuiltinPreviewPicker scene={previewScene} onChange={setPreviewScene} settings={color}
                  iccBase64={iccAttached ? iccBytes?.base64 : null} disabled={submitting} />
              </div>

              {/* 标题 + 昵称 */}
              <div className="grid grid-cols-[1fr_150px] gap-sm">
                <div>
                  <label
                    htmlFor="share-plan-title-input"
                    className="block font-label-md text-label-md text-on-surface-variant mb-2.5"
                  >
                    标题 <span className="text-error">*</span>
                  </label>
                  <input
                    id="share-plan-title-input"
                    value={title}
                    onChange={(e) => { setTitle(e.target.value); setFieldError(null); }}
                    maxLength={200}
                    aria-invalid={fieldError?.field === "title"}
                    className={`${inputClass} ${fieldError?.field === "title" ? "!border-error/70" : ""}`}
                  />
                  {fieldError?.field === "title" ? (
                    <p role="alert" className="mt-1 text-[11px] leading-relaxed text-error">{fieldError.message}</p>
                  ) : null}
                </div>
                <div>
                  <label
                    htmlFor="share-plan-nickname"
                    className="block font-label-md text-label-md text-on-surface-variant mb-2.5"
                  >
                    昵称
                  </label>
                  <input
                    id="share-plan-nickname"
                    value={nickname}
                    onChange={(e) => { setNickname(e.target.value); setFieldError(null); }}
                    maxLength={50}
                    placeholder="匿名"
                    aria-invalid={fieldError?.field === "nickname"}
                    className={`${inputClass} ${fieldError?.field === "nickname" ? "!border-error/70" : ""}`}
                  />
                </div>
              </div>
              {fieldError?.field === "nickname" ? (
                <p role="alert" className="text-[11px] leading-relaxed text-error">{fieldError.message}</p>
              ) : null}

              {/* 说明 */}
              <div>
                <label
                  htmlFor="share-plan-description"
                  className="block font-label-md text-label-md text-on-surface-variant mb-2.5"
                >
                  说明 <span className="text-on-surface-variant/60">（可选）</span>
                </label>
                <textarea
                  id="share-plan-description"
                  value={description}
                  onChange={(e) => { setDescription(e.target.value); setFieldError(null); }}
                  maxLength={2000}
                  rows={2}
                  placeholder="这套参数适合什么场景、什么显示器"
                  aria-invalid={fieldError?.field === "description"}
                  className={`${textareaClass} ${fieldError?.field === "description" ? "!border-error/70" : ""}`}
                />
                {fieldError?.field === "description" ? (
                  <p role="alert" className="mt-1 text-[11px] leading-relaxed text-error">{fieldError.message}</p>
                ) : null}
              </div>

              {/* 参数摘要：只读，紧凑单容器 */}
              <div>
                <label className="block font-label-md text-label-md text-on-surface-variant mb-2.5">
                  将分享的参数
                </label>
                <div className="grid grid-flow-dense grid-cols-3 gap-x-md gap-y-1.5 rounded-md bg-surface-container-high/50 px-md py-sm">
                  {summaryPairs.map(([k, v]) => (
                    <div key={k} className="flex items-baseline justify-between gap-sm min-w-0">
                      <span className="shrink-0 text-[11px] text-on-surface-variant">{k}</span>
                      <span className="truncate font-label-md text-label-md font-medium text-on-surface tabular-nums">
                        {v}
                      </span>
                    </div>
                  ))}
                </div>
              </div>

              {/* ICC 状态：tonal 状态条；图标紧跟标题，状态说明退为次级小字 */}
              <div
                data-name="share-icc-status"
                className={`rounded-md px-md py-sm transition-colors duration-200 ${iccTone.box}`}
              >
                <div className="flex items-start gap-xs min-w-0">
                  <span className="shrink-0 font-label-md text-label-md text-on-surface">ICC 文件</span>
                  {iccState === "uploading" || iccState === "checking" ? (
                    <svg
                      className="h-3.5 w-3.5 animate-spin text-amber-400 motion-reduce:animate-none"
                      viewBox="0 0 24 24"
                      fill="none"
                      aria-hidden
                    >
                      <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                      <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
                    </svg>
                  ) : iccState === "in-library" ? (
                    <Icon name="check_circle" filled className="text-[15px] text-tertiary" />
                  ) : iccState === "failed" ? (
                    <Icon name="error" filled className="text-[15px] text-error" />
                  ) : null}
                  {hasIcc ? (
                    <span
                      className="ml-auto line-clamp-2 break-all text-right font-mono text-[11px] leading-relaxed text-on-surface-variant/70"
                      title={iccName}
                    >
                      {iccName}
                    </span>
                  ) : null}
                </div>
                <p className="mt-1 text-[11px] leading-relaxed text-on-surface-variant line-clamp-2">
                  {iccStatusText}
                </p>
                {iccState === "failed" ? (
                  <div className="mt-sm flex gap-sm">
                    <button
                      onClick={() => {
                        if (iccBytes) {
                          void runIccUpload({
                            base64: iccBytes.base64,
                            sha256: iccBytes.sha256,
                          });
                        }
                      }}
                      className="min-h-9 flex-1 rounded-md border border-primary/40 px-md py-sm text-primary font-label-md text-label-md hover:bg-primary/20 transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-primary"
                    >
                      重试上传
                    </button>
                    <button
                      onClick={() => {
                        setIccNote("上传失败，已按你的选择只分享参数");
                        setIccState("params-only");
                      }}
                      className="min-h-9 flex-1 rounded-md border border-outline-variant/45 px-md py-sm text-on-surface-variant font-label-md text-label-md hover:bg-surface-variant/50 transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-primary"
                    >
                      不传 ICC，仅分享参数
                    </button>
                  </div>
                ) : null}
              </div>
            </div>
          )}
        </div>

        {/* Footer */}
        {link ? null : (
          <div className="relative shrink-0 flex items-center justify-between gap-sm border-t border-outline-variant/15 bg-surface-container px-lg py-md">
            <span className="mr-auto text-[11px] text-on-surface-variant">
              {stage || ""}
            </span>
            <div className="flex gap-sm">
              <button
                onClick={onClose}
                className="min-h-10 px-md py-sm rounded-md text-on-surface-variant font-label-md text-label-md hover:bg-surface-variant/50 transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-primary"
              >
                取消
              </button>
              <button
                onClick={() => void submit()}
                disabled={!canCreate}
                className="min-h-10 px-lg py-sm rounded-md bg-primary text-on-primary font-label-md text-label-md hover:brightness-110 transition-[filter] duration-200 disabled:opacity-50 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary"
              >
                {submitting ? stage || "生成中…" : "生成分享链接"}
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
