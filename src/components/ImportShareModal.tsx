import { useEffect, useState } from "react";
import { invoke } from "../lib/ipc";
import { debugNote } from "../lib/debug";
import {
  getShare,
  parseShareSlug,
  reportShareImport,
  shareAssetUrl,
  type PublicShare,
} from "../lib/share";
import { Icon } from "./Icon";
import { useModalAccessibility } from "../hooks/useModalAccessibility";

interface ImportShareModalProps {
  open: boolean;
  onClose: () => void;
  showToast: (type: "success" | "error", text: string) => void;
  /** 导入成功后通知上层刷新方案列表 */
  onImported: (name: string) => void;
}

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

const inputClass =
  "w-full h-9 px-sm rounded-md bg-surface-container-high border border-outline-variant/25 outline-none focus:border-primary/60 focus-visible:ring-2 focus-visible:ring-primary/30 text-body-sm text-on-surface transition-colors";

export default function ImportShareModal({
  open,
  onClose,
  showToast,
  onImported,
}: ImportShareModalProps) {
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);
  const [share, setShare] = useState<PublicShare | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [importing, setImporting] = useState(false);
  const [stage, setStage] = useState("");
  const [imageFailed, setImageFailed] = useState(false);
  const modalRef = useModalAccessibility(open, onClose, true, !importing);

  useEffect(() => {
    if (!open) return;
    setInput("");
    setShare(null);
    setError(null);
    setStage("");
    setImageFailed(false);
  }, [open]);

  if (!open) return null;

  async function lookup() {
    const slug = parseShareSlug(input);
    if (!slug) {
      setError("这不像一条分享链接，请粘贴完整地址或分享码");
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const item = await getShare(slug);
      if (!item) {
        setError("分享不存在或已被撤销");
        return;
      }
      setShare(item);
      setImageFailed(false);
      debugNote(`查看分享 ${slug}`);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }

  /** 方案名冲突时加后缀，绝不覆盖用户已有方案。 */
  async function uniqueName(base: string): Promise<string> {
    // list_configs 返回完整的方案对象（含 icon 等），这里只关心名字
    const existing = await invoke<Array<{ name?: string }>>("list_configs").catch(
      () => [] as Array<{ name?: string }>,
    );
    const taken = new Set(
      (Array.isArray(existing) ? existing : [])
        .map((item) => (typeof item === "string" ? item : item?.name ?? ""))
        .filter(Boolean),
    );
    const cleaned = base.trim() || "分享的方案";
    if (!taken.has(cleaned)) return cleaned;
    const suffixed = `${cleaned}（分享）`;
    if (!taken.has(suffixed)) return suffixed;
    for (let i = 2; i <= 50; i += 1) {
      const candidate = `${cleaned}（分享 ${i}）`;
      if (!taken.has(candidate)) return candidate;
    }
    return `${cleaned}（${Date.now()}）`;
  }

  async function doImport() {
    if (!share) return;
    setImporting(true);
    try {
      let iccProfile: string | null = null;
      if (share.iccSha256) {
        setStage("下载 ICC 文件");
        try {
          iccProfile = await invoke<string>("import_shared_icc", {
            slug: share.slug,
            fileName: share.iccName,
            title: share.title,
          });
        } catch (err) {
          // ICC 失败不阻断：方案本身仍可导入，但必须说清楚屏幕不会变
          debugNote(`分享 ICC 下载失败：${String(err)}`);
          showToast(
            "error",
            `ICC 下载失败（${err instanceof Error ? err.message : String(err)}），方案仍会导入但缺少 ICC`,
          );
        }
      }

      setStage("保存方案");
      const name = await uniqueName(share.title);
      const config: ColorConfig = {
        name,
        brightness: share.color.brightness,
        contrast: share.color.contrast,
        gamma: share.color.gamma,
        digital_vibrance: share.color.digitalVibrance,
        rgb_r: share.color.rgbR,
        rgb_g: share.color.rgbG,
        rgb_b: share.color.rgbB,
        resolution: share.resolution || null,
        icc_profile: iccProfile,
      };
      await invoke("save_config", { config });
      void reportShareImport(share.slug);
      debugNote(`导入分享方案「${name}」`);
      showToast("success", `已导入「${name}」`);
      onImported(name);
      onClose();
    } catch (err) {
      showToast("error", err instanceof Error ? err.message : String(err));
    } finally {
      setImporting(false);
      setStage("");
    }
  }

  const coverUrl =
    share && (share.thumbKey || share.coverKey)
      ? shareAssetUrl(share.thumbKey || share.coverKey)
      : "";

  const summaryPairs: Array<[string, string]> = share
    ? [
        ["亮度", String(share.color.brightness)],
        ["对比度", String(share.color.contrast)],
        ["Gamma", String(share.color.gamma)],
        ["数字振动", String(share.color.digitalVibrance)],
        ["RGB", `${share.color.rgbR} / ${share.color.rgbG} / ${share.color.rgbB}`],
        ["分辨率", share.resolution || "不调整"],
      ]
    : [];

  return (
    <div
      className="fixed inset-0 z-[300] flex items-center justify-center bg-black/60 p-3 backdrop-blur-[2px] sm:p-5"
      onClick={onClose}
    >
      <div
        ref={modalRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="import-share-title"
        tabIndex={-1}
        onClick={(e) => e.stopPropagation()}
        className="relative bg-surface-container rounded-xl shadow-2xl border border-outline-variant/25 w-[520px] max-w-[94vw] max-h-[90vh] flex flex-col overflow-hidden animate-[slide-fade-up_0.28s_cubic-bezier(0.16,1,0.3,1)_forwards] motion-reduce:animate-none"
      >
        {/* 环境景深：与分享弹窗同款头部洗染 */}
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
            <h2 id="import-share-title" className="font-headline-sm text-headline-sm text-on-surface">
              导入分享链接
            </h2>
            <p className="mt-xs text-body-sm text-on-surface-variant">
              粘贴别人分享的地址或分享码
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
          <div className="px-lg py-lg space-y-lg">
          <div className="flex gap-sm">
            <input
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") void lookup();
              }}
              placeholder="https://.../s/xxxxxxxx"
              aria-label="分享链接"
              className={inputClass}
            />
            <button
              onClick={() => void lookup()}
              disabled={loading || !input.trim()}
              className="min-h-9 shrink-0 px-lg rounded-md border border-primary/35 bg-primary/15 text-primary font-label-md text-label-md hover:bg-primary/25 hover:border-primary/50 transition-colors disabled:opacity-40 focus-visible:outline focus-visible:outline-2 focus-visible:outline-primary"
            >
              {loading ? "查询中" : "查询"}
            </button>
          </div>

          {error ? (
            <div className="rounded-md bg-error/10 px-md py-sm text-body-sm text-error">{error}</div>
          ) : null}

          {share ? (
            <div className="group overflow-hidden rounded-md border border-outline-variant/20 bg-surface-container-high/40">
              {coverUrl && !imageFailed ? (
                <div className="aspect-[16/9] max-h-44 overflow-hidden bg-surface-container-high">
                  <img
                    src={coverUrl}
                    alt={share.title}
                    className="h-full w-full object-cover transition-transform duration-500 ease-out group-hover:scale-[1.03] motion-reduce:transition-none motion-reduce:group-hover:scale-100"
                    onError={() => setImageFailed(true)}
                  />
                </div>
              ) : null}
              <div className="p-md space-y-sm">
                <div>
                  <div className="font-title-sm text-title-sm text-on-surface">{share.title}</div>
                  <div className="mt-1 text-[11px] text-on-surface-variant">
                    {share.contributorName ? `由 ${share.contributorName} 分享 · ` : ""}
                    {share.importCount} 次导入
                  </div>
                </div>
                {share.description ? (
                  <p className="text-body-sm text-on-surface-variant whitespace-pre-wrap">
                    {share.description}
                  </p>
                ) : null}
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
                <div className="flex items-center gap-xs text-[11px] text-on-surface-variant">
                  <Icon name={share.iccName ? "download" : "info"} className="text-[14px]" />
                  {share.iccName
                    ? `含 ICC：${share.iccName}`
                    : "只含色彩参数，不含 ICC 文件"}
                </div>
              </div>
            </div>
          ) : null}
          </div>
        </div>

        {/* Footer */}
        <div className="relative shrink-0 flex items-center justify-between gap-sm border-t border-outline-variant/15 bg-surface-container px-lg py-md">
          <span className="mr-auto text-[11px] text-on-surface-variant">
            {stage || "导入不会覆盖你已有的同名方案"}
          </span>
          <div className="flex gap-sm">
            <button
              onClick={onClose}
              className="min-h-10 px-md py-sm rounded-md text-on-surface-variant font-label-md text-label-md hover:bg-surface-variant/50 transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-primary"
            >
              取消
            </button>
            <button
              onClick={() => void doImport()}
              disabled={!share || importing}
              className="min-h-10 px-lg py-sm rounded-md bg-primary text-on-primary font-label-md text-label-md hover:brightness-110 transition-[filter] duration-200 disabled:opacity-50 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary"
            >
              {importing ? stage || "导入中…" : "导入这条方案"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
