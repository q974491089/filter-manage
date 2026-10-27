import { useEffect, useRef, useState } from "react";
import { Icon } from "./Icon";
import { useModalAccessibility } from "../hooks/useModalAccessibility";
import { debugNote } from "../lib/debug";
import { prepareCover } from "../lib/imageCompression";
import {
  createSubmission,
  getContributorId,
  getSubmissionTokens,
  sha256Hex,
  uploadToGateway,
} from "../lib/submissions";

interface SubmitIccModalProps {
  open: boolean;
  onClose: () => void;
  onSubmitted: () => void;
  showToast: (type: "success" | "error", text: string) => void;
  iccEffectPreview: string | null;
}

export default function SubmitIccModal({
  open,
  onClose,
  onSubmitted,
  showToast,
  iccEffectPreview,
}: SubmitIccModalProps) {
  const [file, setFile] = useState<File | null>(null);
  const [cover, setCover] = useState<File | null>(null);
  const [coverPreviewUrl, setCoverPreviewUrl] = useState("");
  const [title, setTitle] = useState("");
  const [panel, setPanel] = useState("");
  const [tags, setTags] = useState("");
  const [description, setDescription] = useState("");
  const [nickname, setNickname] = useState("");
  const [stage, setStage] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const coverInputRef = useRef<HTMLInputElement>(null);
  const modalRef = useModalAccessibility(open, onClose, true, !submitting);

  useEffect(() => {
    if (!cover) {
      setCoverPreviewUrl("");
      return;
    }
    const url = URL.createObjectURL(cover);
    setCoverPreviewUrl(url);
    return () => URL.revokeObjectURL(url);
  }, [cover]);

  useEffect(() => {
    if (!open || !iccEffectPreview) return;
    void fetch(`data:image/png;base64,${iccEffectPreview}`)
      .then((response) => response.blob())
      .then((blob) => setCover(new File([blob], "icc-effect-preview.png", { type: "image/png" })))
      .catch((error) => showToast("error", `读取模拟效果图失败：${String(error)}`));
  }, [open, iccEffectPreview, showToast]);

  if (!open) return null;

  const reset = () => {
    setFile(null);
    setCover(null);
    if (coverInputRef.current) coverInputRef.current.value = "";
    setTitle("");
    setPanel("");
    setTags("");
    setDescription("");
    setNickname("");
    setStage("");
  };

  async function submit() {
    if (!file) {
      showToast("error", "请选择 ICC 文件");
      return;
    }
    if (!title.trim()) {
      showToast("error", "请填写名称");
      return;
    }

    setSubmitting(true);
    try {
      const buffer = await file.arrayBuffer();
      const sha = await sha256Hex(buffer);
      const contributorId = await getContributorId();
      const tokens = await getSubmissionTokens(contributorId, nickname.trim());

      setStage("上传 ICC 文件");
      await uploadToGateway(
        `${tokens.gatewayUrl}/v1/icc`,
        tokens.iccToken,
        sha,
        buffer,
        "application/vnd.iccprofile",
      );

      let coverKey = "";
      let thumbKey = "";
      if (cover) {
        setStage("压缩并上传封面");
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

      setStage("提交审核");
      await createSubmission({
        contributorId,
        nickname: nickname.trim(),
        sha256: sha,
        title: title.trim(),
        description: description.trim(),
        panel: panel.trim(),
        tags: tags
          .split(/[,，]/)
          .map((tag) => tag.trim())
          .filter(Boolean),
        coverKey,
        thumbKey,
        clientVersion: __APP_VERSION__,
      });
      debugNote(`投稿 ICC「${title.trim()}」`);
      showToast("success", "已提交，等待管理员审核");
      reset();
      onSubmitted();
      onClose();
    } catch (err) {
      showToast("error", err instanceof Error ? err.message : "提交失败");
    } finally {
      setSubmitting(false);
      setStage("");
    }
  }

  return (
    <div
      data-components="SubmitIccModal"
      className="fixed inset-0 z-[320] flex items-center justify-center bg-black/65 p-3 backdrop-blur-[2px] sm:p-5"
      onClick={() => !submitting && onClose()}
    >
      <div
        ref={modalRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="icc-submit-title"
        aria-describedby="icc-submit-description"
        tabIndex={-1}
        onClick={(e) => e.stopPropagation()}
        className="bg-surface-container rounded-xl shadow-2xl border border-outline-variant/25 w-[560px] max-w-[94vw] max-h-[90vh] overflow-y-auto"
      >
        <div className="sticky top-0 z-10 flex items-center justify-between gap-md border-b border-outline-variant/15 bg-surface-container px-lg py-md">
          <div>
          <h2 id="icc-submit-title" className="font-headline-sm text-headline-sm text-on-surface">
            提供 ICC 配置
          </h2>
          <p id="icc-submit-description" className="mt-1 text-body-sm text-on-surface-variant">提交后由管理员审核，通过后会显示在市场中。</p>
          </div>
          <button
            onClick={() => !submitting && onClose()}
            aria-label="关闭"
            className="grid h-9 w-9 shrink-0 place-items-center rounded-md text-on-surface-variant hover:bg-surface-variant/50 hover:text-on-surface transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-primary"
          >
            <Icon name="close" className="text-[20px]" />
          </button>
        </div>

        <div className="space-y-md px-lg py-md">
        <div>
        <label htmlFor="icc-submit-file" className="block font-label-md text-label-md text-on-surface-variant mb-xs">
          ICC 文件
        </label>
        <input
          id="icc-submit-file"
          type="file"
          accept=".icc,.icm"
          onChange={(e) => setFile(e.target.files?.[0] ?? null)}
          className="block w-full rounded-md border border-outline-variant/25 bg-surface-container-high p-sm text-body-sm text-on-surface-variant file:mr-sm file:rounded file:border-0 file:bg-surface-variant file:px-sm file:py-xs file:text-on-surface focus-visible:outline focus-visible:outline-2 focus-visible:outline-primary"
        />
        </div>

        <div>
        <label htmlFor="icc-submit-title-input" className="block font-label-md text-label-md text-on-surface-variant mb-xs">
          名称 <span className="text-error">*</span>
        </label>
        <input
          id="icc-submit-title-input"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          maxLength={200}
          required
          placeholder="例如：日常观影校准"
          className="w-full px-sm py-sm rounded-md bg-surface-container-high border border-outline-variant/25 outline-none focus:border-primary/60 focus-visible:ring-2 focus-visible:ring-primary/30 text-on-surface"
        />
        </div>

        <div>
        <label htmlFor="icc-submit-panel" className="block font-label-md text-label-md text-on-surface-variant mb-xs">
          显示器型号
        </label>
        <input
          id="icc-submit-panel"
          value={panel}
          onChange={(e) => setPanel(e.target.value)}
          maxLength={100}
          placeholder="例如：Dell U2723QE"
          className="w-full px-sm py-sm rounded-md bg-surface-container-high border border-outline-variant/25 outline-none focus:border-primary/60 focus-visible:ring-2 focus-visible:ring-primary/30 text-on-surface"
        />
        </div>

        <div>
        <label htmlFor="icc-submit-tags" className="block font-label-md text-label-md text-on-surface-variant mb-xs">
          标签（逗号分隔）
        </label>
        <input
          id="icc-submit-tags"
          value={tags}
          onChange={(e) => setTags(e.target.value)}
          placeholder="例如：sRGB，护眼，摄影"
          className="w-full px-sm py-sm rounded-md bg-surface-container-high border border-outline-variant/25 outline-none focus:border-primary/60 focus-visible:ring-2 focus-visible:ring-primary/30 text-on-surface"
        />
        </div>

        <div>
        <label htmlFor="icc-submit-description-input" className="block font-label-md text-label-md text-on-surface-variant mb-xs">
          说明
        </label>
        <textarea
          id="icc-submit-description-input"
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          rows={3}
          maxLength={5000}
          placeholder="说明适用场景、校准设备或使用建议"
          className="w-full px-sm py-sm rounded-md bg-surface-container-high border border-outline-variant/25 outline-none focus:border-primary/60 focus-visible:ring-2 focus-visible:ring-primary/30 text-on-surface resize-y"
        />
        </div>

        <div>
        <label htmlFor="icc-submit-nickname" className="block font-label-md text-label-md text-on-surface-variant mb-xs">
          昵称（可选）
        </label>
        <input
          id="icc-submit-nickname"
          value={nickname}
          onChange={(e) => setNickname(e.target.value)}
          maxLength={50}
          placeholder="留空则匿名展示"
          className="w-full px-sm py-sm rounded-md bg-surface-container-high border border-outline-variant/25 outline-none focus:border-primary/60 focus-visible:ring-2 focus-visible:ring-primary/30 text-on-surface"
        />
        </div>

        <div>
        <label htmlFor="icc-submit-cover" className="block font-label-md text-label-md text-on-surface-variant mb-xs">
          效果图（可选，一张）
        </label>
        <input
          ref={coverInputRef}
          id="icc-submit-cover"
          type="file"
          accept="image/*"
          onChange={(e) => setCover(e.target.files?.[0] ?? null)}
          className="block w-full rounded-md border border-outline-variant/25 bg-surface-container-high p-sm text-body-sm text-on-surface-variant file:mr-sm file:rounded file:border-0 file:bg-surface-variant file:px-sm file:py-xs file:text-on-surface focus-visible:outline focus-visible:outline-2 focus-visible:outline-primary"
        />
        {coverPreviewUrl ? (
          <div className="mt-sm overflow-hidden rounded-md border border-outline-variant/20 bg-surface-container-high">
            <div className="relative aspect-[16/9] max-h-56">
              <img
                src={coverPreviewUrl}
                alt={`效果图预览：${cover?.name ?? ""}`}
                className="h-full w-full object-cover"
              />
              <button
                type="button"
                onClick={() => {
                  setCover(null);
                  if (coverInputRef.current) coverInputRef.current.value = "";
                }}
                disabled={submitting}
                aria-label="移除效果图"
                title="移除效果图"
                className="absolute right-sm top-sm grid h-8 w-8 place-items-center rounded-md bg-black/65 text-white transition-colors hover:bg-black/80 disabled:opacity-50 focus-visible:outline focus-visible:outline-2 focus-visible:outline-white"
              >
                <Icon name="close" className="text-[18px]" />
              </button>
            </div>
            <p className="truncate px-sm py-xs text-xs text-on-surface-variant" title={cover?.name}>
              {cover?.name}
            </p>
          </div>
        ) : null}
        </div>

        {stage ? (
          <p role="status" aria-live="polite" className="flex items-center gap-sm rounded-md bg-primary/10 px-md py-sm font-label-md text-label-md text-primary">
            <span className="h-2 w-2 animate-pulse rounded-full bg-primary motion-reduce:animate-none" />
            {stage}…
          </p>
        ) : null}
        </div>

        <div className="sticky bottom-0 flex justify-end gap-sm border-t border-outline-variant/15 bg-surface-container px-lg py-md">
          <button
            onClick={() => !submitting && onClose()}
            disabled={submitting}
            className="min-h-10 px-md py-sm rounded-md text-on-surface-variant hover:bg-surface-variant/50 disabled:opacity-50 focus-visible:outline focus-visible:outline-2 focus-visible:outline-primary"
          >
            取消
          </button>
          <button
            onClick={() => void submit()}
            disabled={submitting}
            className="min-h-10 px-lg py-sm rounded-md bg-primary text-on-primary font-label-md text-label-md hover:brightness-110 transition-[filter] duration-200 disabled:opacity-50 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary"
          >
            {submitting ? "提交中…" : "提交审核"}
          </button>
        </div>
      </div>
    </div>
  );
}
