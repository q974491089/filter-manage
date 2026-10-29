import { useEffect, useMemo, useRef, useState } from "react";
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
  /** 当前是否已有可用的模拟效果图（data:image/png;base64,...） */
  iccEffectPreview: string | null;
  /** 生成模拟效果图（复用顶栏相机入口的实现） */
  onCaptureEffect: () => Promise<string | null>;
}

type IccState = "none" | "checking" | "in-library" | "missing" | "no-file";

export default function SharePlanModal({
  open,
  config,
  onClose,
  showToast,
  iccEffectPreview,
  onCaptureEffect,
}: SharePlanModalProps) {
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [nickname, setNickname] = useState("");
  const [cover, setCover] = useState<File | null>(null);
  const [coverPreviewUrl, setCoverPreviewUrl] = useState("");
  const [iccState, setIccState] = useState<IccState>("none");
  const [iccBytes, setIccBytes] = useState<{ base64: string; sha256: string; size: number } | null>(null);
  const [uploading, setUploading] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [stage, setStage] = useState("");
  const [link, setLink] = useState("");
  const coverInputRef = useRef<HTMLInputElement>(null);
  const modalRef = useModalAccessibility(open, onClose, true, !submitting);

  const iccName = config?.icc_profile ?? "";
  const hasIcc = Boolean(iccName);

  // 打开时重置并开始检测 ICC 是否已在库里
  useEffect(() => {
    if (!open || !config) return;
    setTitle(config.name);
    setDescription("");
    setNickname("");
    setCover(null);
    setIccState(hasIcc ? "checking" : "none");
    setIccBytes(null);
    setLink("");
    setStage("");
    setUploading(false);
    setSubmitting(false);
  }, [open, config, hasIcc]);

  // 自动带入模拟效果图作为封面（与投稿弹窗同一策略）
  useEffect(() => {
    if (!open || !iccEffectPreview) return;
    void fetch(`data:image/png;base64,${iccEffectPreview}`)
      .then((r) => r.blob())
      .then((blob) => setCover(new File([blob], "effect-preview.png", { type: "image/png" })))
      .catch(() => {
        /* 封面只是可选内容，失败静默 */
      });
  }, [open, iccEffectPreview]);

  useEffect(() => {
    if (!cover) {
      setCoverPreviewUrl("");
      return;
    }
    const url = URL.createObjectURL(cover);
    setCoverPreviewUrl(url);
    return () => URL.revokeObjectURL(url);
  }, [cover]);

  // ICC 检测：读本地字节 → 算 sha → 问服务端在不在库
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
        if (!cancelled) setIccState(exists ? "in-library" : "missing");
      } catch (err) {
        if (cancelled) return;
        debugNote(`分享检测 ICC 失败：${String(err)}`);
        setIccState("no-file");
      } finally {
        if (!cancelled) setStage("");
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [open, hasIcc, iccName]);

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

  if (!open || !config) return null;

  const canCreate =
    !submitting &&
    !uploading &&
    title.trim().length > 0 &&
    (iccState === "none" || iccState === "in-library" || iccState === "no-file");

  async function uploadIcc() {
    if (!iccBytes) return;
    setUploading(true);
    try {
      setStage("上传 ICC 文件");
      const contributorId = await getContributorId();
      const tokens = await getSubmissionTokens(contributorId, nickname.trim());
      const binary = atob(iccBytes.base64);
      const buffer = new Uint8Array(binary.length);
      for (let i = 0; i < binary.length; i += 1) buffer[i] = binary.charCodeAt(i);
      await uploadToGateway(
        `${tokens.gatewayUrl}/v1/icc`,
        tokens.iccToken,
        iccBytes.sha256,
        buffer.buffer,
        "application/vnd.iccprofile",
      );
      setIccState("in-library");
      showToast("success", "ICC 文件已上传到库");
      debugNote(`分享上传 ICC「${iccName}」`);
    } catch (err) {
      showToast("error", err instanceof Error ? err.message : String(err));
    } finally {
      setUploading(false);
      setStage("");
    }
  }

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
      const usableIcc = iccState === "in-library" && iccBytes;
      const result = await createShare({
        title: title.trim(),
        description: description.trim(),
        color,
        resolution: config?.resolution ?? "",
        iccSha256: usableIcc ? iccBytes.sha256 : "",
        iccName: usableIcc ? iccName : "",
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
        return "文件已在库中，无需重复上传";
      case "missing":
        return "文件还不在库中，需要先上传";
      case "no-file":
        return "本机找不到这个 ICC 文件，只能分享参数";
      default:
        return "这条方案不包含 ICC 文件，只分享色彩参数与分辨率";
    }
  })();

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
        className="bg-surface-container rounded-xl shadow-2xl border border-outline-variant/25 w-[560px] max-w-[94vw] max-h-[90vh] flex flex-col overflow-hidden"
      >
        <div className="flex items-start justify-between gap-md px-lg pt-lg pb-md shrink-0 border-b border-outline-variant/15">
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

        <div className="flex-1 min-h-0 overflow-y-auto px-lg py-md space-y-md">
          {link ? (
            <div className="space-y-md">
              <div className="flex items-center gap-sm text-primary font-label-md">
                <Icon name="check_circle" filled className="text-[18px]" />
                分享链接已生成
              </div>
              <div className="px-md py-sm rounded-lg bg-surface-container-high border border-outline-variant/25 break-all font-label-md text-label-md">
                {link}
              </div>
              <div className="flex gap-sm">
                <button
                  onClick={() => void invoke("copy_text", { text: link })}
                  className="flex-1 py-sm rounded-md bg-primary text-on-primary font-label-md text-label-md hover:brightness-110 transition"
                >
                  再次复制
                </button>
                <button
                  onClick={onClose}
                  className="flex-1 py-sm rounded-md bg-surface-variant/60 text-on-surface-variant font-label-md text-label-md hover:bg-surface-variant transition"
                >
                  完成
                </button>
              </div>
            </div>
          ) : (
            <>
              <div>
                <label className="font-label-md text-label-md text-on-surface-variant">标题</label>
                <input
                  value={title}
                  onChange={(e) => setTitle(e.target.value)}
                  maxLength={200}
                  className="mt-xs w-full px-md h-10 rounded-lg bg-surface-container-high border border-outline-variant/25 focus:border-primary/60 outline-none font-body-md text-body-md"
                />
              </div>

              <div>
                <label className="font-label-md text-label-md text-on-surface-variant">
                  说明（可选）
                </label>
                <textarea
                  value={description}
                  onChange={(e) => setDescription(e.target.value)}
                  maxLength={2000}
                  rows={2}
                  placeholder="这套参数适合什么场景、什么显示器"
                  className="mt-xs w-full px-md py-sm rounded-lg bg-surface-container-high border border-outline-variant/25 focus:border-primary/60 outline-none font-body-sm text-body-sm resize-none"
                />
              </div>

              <div className="grid grid-cols-3 gap-sm">
                {[
                  ["亮度", color.brightness],
                  ["对比度", color.contrast],
                  ["Gamma", color.gamma],
                  ["数字振动", color.digitalVibrance],
                  ["RGB", `${color.rgbR}/${color.rgbG}/${color.rgbB}`],
                  ["分辨率", config?.resolution || "不调整"],
                ].map(([k, v]) => (
                  <div key={String(k)} className="px-sm py-2 rounded-lg bg-surface-container-high/60">
                    <div className="text-[11px] text-on-surface-variant">{k}</div>
                    <div className="font-label-md text-label-md text-on-surface">{v}</div>
                  </div>
                ))}
              </div>

              <div>
                <label className="font-label-md text-label-md text-on-surface-variant">
                  效果图（可选）
                </label>
                <div className="mt-xs flex items-center gap-sm">
                  {coverPreviewUrl ? (
                    <img
                      src={coverPreviewUrl}
                      alt="效果图预览"
                      className="w-24 h-16 object-cover rounded-md border border-outline-variant/25"
                    />
                  ) : (
                    <div className="w-24 h-16 grid place-items-center rounded-md bg-surface-container-high text-on-surface-variant/50">
                      <Icon name="image" className="text-[20px]" />
                    </div>
                  )}
                  <div className="flex flex-col gap-1">
                    <button
                      onClick={() => coverInputRef.current?.click()}
                      className="px-sm py-1 rounded-md text-primary font-label-sm text-label-sm hover:bg-primary/10 transition"
                    >
                      选择图片
                    </button>
                    <button
                      onClick={() => {
                        void onCaptureEffect().then((data) => {
                          if (!data) return;
                          void fetch(`data:image/png;base64,${data}`)
                            .then((r) => r.blob())
                            .then((b) => setCover(new File([b], "effect-preview.png", { type: "image/png" })));
                        });
                      }}
                      className="px-sm py-1 rounded-md text-on-surface-variant font-label-sm text-label-sm hover:bg-surface-variant/50 transition"
                    >
                      用当前参数生成
                    </button>
                    {cover ? (
                      <button
                        onClick={() => setCover(null)}
                        className="px-sm py-1 rounded-md text-error font-label-sm text-label-sm hover:bg-error/10 transition"
                      >
                        移除
                      </button>
                    ) : null}
                  </div>
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
              </div>

              <div className="px-md py-sm rounded-lg border border-outline-variant/25 bg-surface-container-high/40">
                <div className="flex items-center gap-xs font-label-md text-label-md">
                  <Icon
                    name={iccState === "in-library" ? "check_circle" : "info"}
                    className={iccState === "in-library" ? "text-primary" : "text-on-surface-variant"}
                  />
                  ICC 文件
                </div>
                <p className="mt-xs text-body-sm text-on-surface-variant">{iccStatusText}</p>
                {hasIcc ? (
                  <p className="mt-1 text-[11px] text-on-surface-variant/70 break-all">{iccName}</p>
                ) : null}
                {iccState === "missing" && iccBytes ? (
                  <button
                    onClick={() => void uploadIcc()}
                    disabled={uploading}
                    className="mt-sm w-full py-sm rounded-md bg-primary text-on-primary font-label-md text-label-md hover:brightness-110 transition disabled:opacity-50"
                  >
                    {uploading ? stage || "上传中…" : "上传到文件库"}
                  </button>
                ) : null}
              </div>

              <div>
                <label className="font-label-md text-label-md text-on-surface-variant">
                  昵称（可选，显示在分享页）
                </label>
                <input
                  value={nickname}
                  onChange={(e) => setNickname(e.target.value)}
                  maxLength={50}
                  placeholder="匿名"
                  className="mt-xs w-full px-md h-10 rounded-lg bg-surface-container-high border border-outline-variant/25 focus:border-primary/60 outline-none font-body-md text-body-md"
                />
              </div>
            </>
          )}
        </div>

        {link ? null : (
          <div className="shrink-0 px-lg py-md border-t border-outline-variant/15 flex items-center justify-between gap-sm">
            <span className="text-[11px] text-on-surface-variant">
              {stage || "链接生成后可随时在后台撤销"}
            </span>
            <div className="flex gap-sm">
              <button
                onClick={onClose}
                className="px-md py-sm rounded-md text-on-surface-variant font-label-md text-label-md hover:bg-surface-variant/50 transition"
              >
                取消
              </button>
              <button
                onClick={() => void submit()}
                disabled={!canCreate}
                className="px-md py-sm rounded-md bg-primary text-on-primary font-label-md text-label-md hover:brightness-110 transition disabled:opacity-40"
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
