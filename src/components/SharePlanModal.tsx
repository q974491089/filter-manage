import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "../lib/ipc";
import { debugNote } from "../lib/debug";
import { prepareCover } from "../lib/imageCompression";
import { sha256Hex, getContributorId, getSubmissionTokens, uploadToGateway } from "../lib/submissions";
import { checkShareIcc, createShare, shareUrl, type ShareColor } from "../lib/share";
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
  /** 生成模拟效果图（用**被分享方案**的参数，不是当前屏幕参数） */
  onCaptureEffect: (config: ColorConfig) => Promise<string | null>;
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
  onCaptureEffect,
}: SharePlanModalProps) {
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [nickname, setNickname] = useState("");
  const [cover, setCover] = useState<File | null>(null);
  const [coverPreviewUrl, setCoverPreviewUrl] = useState("");
  const [iccState, setIccState] = useState<IccState>("none");
  const [iccNote, setIccNote] = useState("");
  const [iccBytes, setIccBytes] = useState<{ base64: string; sha256: string; size: number } | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [stage, setStage] = useState("");
  const [link, setLink] = useState("");
  const coverInputRef = useRef<HTMLInputElement>(null);
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
    setCover(null);
    setIccState(hasIcc ? "checking" : "none");
    setIccNote("");
    setIccBytes(null);
    setLink("");
    setStage("");
    setSubmitting(false);
  }, [open, config, hasIcc]);

  // 注意：不在打开时自动塞入效果图。顶栏那张模拟图是按「当前屏幕参数」生成的，
  // 与被分享的方案可能不同；效果图必须由用户点「用这套参数生成」显式产生，
  // 避免把无关画面当成这套方案的预览发出去。


  useEffect(() => {
    if (!cover) {
      setCoverPreviewUrl("");
      return;
    }
    const url = URL.createObjectURL(cover);
    setCoverPreviewUrl(url);
    return () => URL.revokeObjectURL(url);
  }, [cover]);

  const nicknameRef = useRef("");
  nicknameRef.current = nickname;

  /**
   * 把 ICC 字节传到内容寻址网关。稳定引用（nickname 走 ref），
   * 检测流程和手动重试共用，避免依赖变化触发重新检测。
   */
  const runIccUpload = useCallback(
    async (bytes: { base64: string; sha256: string }): Promise<boolean> => {
      setIccState("uploading");
      setStage("自动上传 ICC 文件到库");
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
        setStage("检查 ICC 文件");
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
      let coverKey = "";
      let thumbKey = "";
      const contributorId = await getContributorId();

      if (cover) {
        setStage("压缩并上传效果图");
        const tokens = await getSubmissionTokens(contributorId, nickname.trim());
        const prepared = await prepareCover(cover);
        const images: Array<["cover" | "thumb", File]> = [
          ["cover", prepared.cover],
          ["thumb", prepared.thumb],
        ];
        for (const [kind, image] of images) {
          const imgBuffer = await image.arrayBuffer();
          const imgSha = await sha256Hex(imgBuffer);
          const result = await uploadToGateway(
            `${tokens.imageGatewayUrl}/v1/images`,
            tokens.imageToken,
            imgSha,
            imgBuffer,
            "image/webp",
          );
          if (kind === "cover") coverKey = result.key;
          else thumbKey = result.key;
        }
      }

      setStage("创建分享链接");
      const result = await createShare({
        title: title.trim(),
        description: description.trim(),
        color,
        resolution: config?.resolution ?? "",
        iccSha256: iccAttached ? iccBytes!.sha256 : "",
        iccName: iccAttached ? iccName : "",
        coverKey,
        thumbKey,
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
      setLink(url);
    } catch (err) {
      showToast("error", err instanceof Error ? err.message : String(err));
    } finally {
      setSubmitting(false);
      setStage("");
    }
  }

  const iccStatusText = (() => {
    switch (iccState) {
      case "checking":
        return stage || "正在检查 ICC 文件…";
      case "in-library":
        return "已在库中，导入方会自动下载";
      case "uploading":
        return stage || "正在自动上传 ICC 文件…";
      case "failed":
        return `自动上传失败：${iccNote}`;
      case "params-only":
        return iccNote || "无法附带这个 ICC 文件，只能分享色彩参数";
      default:
        return "这条方案不包含 ICC 文件，只分享色彩参数与分辨率";
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
    <div
      className="fixed inset-0 z-[300] flex items-center justify-center bg-black/60 p-3 backdrop-blur-[2px] sm:p-5"
      onClick={onClose}
    >
      <div
        ref={modalRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="share-plan-title"
        tabIndex={-1}
        onClick={(e) => e.stopPropagation()}
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
                  className="min-h-10 flex-1 rounded-md bg-primary/12 px-md py-sm text-primary font-label-md text-label-md hover:bg-primary/20 transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-primary"
                >
                  复制链接
                </button>
                <button
                  onClick={onClose}
                  className="min-h-10 flex-1 rounded-md border border-outline-variant/30 px-md py-sm text-on-surface-variant font-label-md text-label-md hover:bg-surface-variant/50 transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-primary"
                >
                  完成
                </button>
              </div>
            </div>
          ) : (
            <div className="px-lg py-lg space-y-xl">
              {/* 效果图：视觉锚点，16:9 横幅 */}
              <div>
                <label className="block font-label-md text-label-md text-on-surface-variant mb-2.5">
                  效果图 <span className="text-on-surface-variant/60">（可选）</span>
                </label>
                {cover ? (
                  <div className="overflow-hidden rounded-md border border-outline-variant/20 bg-surface-container-high">
                    {/* 固定高度 + object-cover：避免 aspect-ratio 与 max-h 互相压制，
                        导致图片不铺满、右上角按钮看着"飘"在画面中间 */}
                    <div className="group relative h-44 w-full overflow-hidden bg-surface-container-highest">
                      <img
                        src={coverPreviewUrl}
                        alt="效果图预览"
                        className="h-full w-full object-cover transition-transform duration-500 ease-out group-hover:scale-[1.03] motion-reduce:transition-none motion-reduce:group-hover:scale-100"
                      />
                      <div
                        aria-hidden
                        className="pointer-events-none absolute inset-x-0 top-0 h-16 bg-gradient-to-b from-black/50 to-transparent"
                      />
                      <button
                        onClick={() => setCover(null)}
                        aria-label="移除效果图"
                        title="移除效果图"
                        className="absolute right-2 top-2 z-10 grid h-7 w-7 place-items-center rounded-full bg-black/60 text-white ring-1 ring-white/25 transition-colors hover:bg-black/80 focus-visible:outline focus-visible:outline-2 focus-visible:outline-white"
                      >
                        <Icon name="close" className="text-[16px]" />
                      </button>
                    </div>
                    <p className="truncate px-sm py-xs text-xs text-on-surface-variant" title={cover.name}>
                      {cover.name === "effect-preview.png" ? "按这套方案模拟生成的效果图" : cover.name}
                    </p>
                  </div>
                ) : (
                  <div className="flex items-center gap-sm rounded-md border border-dashed border-outline-variant/30 bg-surface-container-high/40 px-md py-sm">
                    <Icon name="image" className="text-[20px] text-on-surface-variant/60" />
                    <p className="mr-auto text-body-sm text-on-surface-variant">
                      给接收方看调校后的效果
                    </p>
                    <button
                      onClick={() => {
                        void onCaptureEffect(config).then((data) => {
                          if (!data) return;
                          void fetch(`data:image/png;base64,${data}`)
                            .then((r) => r.blob())
                            .then((b) =>
                              setCover(new File([b], "effect-preview.png", { type: "image/png" })),
                            );
                        });
                      }}
                      className="min-h-9 rounded-md bg-primary/12 px-md py-sm text-primary font-label-md text-label-md hover:bg-primary/20 transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-primary"
                    >
                      用这套参数生成
                    </button>
                    <button
                      onClick={() => coverInputRef.current?.click()}
                      className="min-h-9 rounded-md border border-outline-variant/30 px-md py-sm text-on-surface-variant font-label-md text-label-md hover:bg-surface-variant/50 transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-primary"
                    >
                      选择图片
                    </button>
                    <input
                      ref={coverInputRef}
                      type="file"
                      accept="image/*"
                      className="hidden"
                      onChange={(e) => {
                        const file = e.target.files?.[0];
                        if (file) setCover(file);
                      }}
                    />
                  </div>
                )}
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
                    onChange={(e) => setTitle(e.target.value)}
                    maxLength={200}
                    className={inputClass}
                  />
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
                    onChange={(e) => setNickname(e.target.value)}
                    maxLength={50}
                    placeholder="匿名"
                    className={inputClass}
                  />
                </div>
              </div>

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
                  onChange={(e) => setDescription(e.target.value)}
                  maxLength={2000}
                  rows={2}
                  placeholder="这套参数适合什么场景、什么显示器"
                  className={textareaClass}
                />
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
                <div className="flex items-center gap-xs min-w-0">
                  <span className="font-label-md text-label-md text-on-surface">ICC 文件</span>
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
                      className="ml-auto truncate font-mono text-[11px] text-on-surface-variant/70"
                      title={iccName}
                    >
                      {iccName}
                    </span>
                  ) : null}
                </div>
                <p className="mt-1 text-[11px] leading-relaxed text-on-surface-variant">
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
                      className="min-h-9 flex-1 rounded-md bg-primary/12 px-md py-sm text-primary font-label-md text-label-md hover:bg-primary/20 transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-primary"
                    >
                      重试上传
                    </button>
                    <button
                      onClick={() => {
                        setIccNote("上传失败，已按你的选择只分享参数");
                        setIccState("params-only");
                      }}
                      className="min-h-9 flex-1 rounded-md border border-outline-variant/30 px-md py-sm text-on-surface-variant font-label-md text-label-md hover:bg-surface-variant/50 transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-primary"
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
              链接生成后可随时在后台撤销
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
