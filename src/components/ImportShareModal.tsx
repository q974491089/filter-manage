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
        className="bg-surface-container rounded-xl shadow-2xl border border-outline-variant/25 w-[520px] max-w-[94vw] max-h-[90vh] flex flex-col overflow-hidden"
      >
        <div className="flex items-start justify-between gap-md px-lg pt-lg pb-md shrink-0 border-b border-outline-variant/15">
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

        <div className="flex-1 min-h-0 overflow-y-auto px-lg py-md space-y-md">
          <div className="flex gap-sm">
            <input
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") void lookup();
              }}
              placeholder="https://.../s/xxxxxxxx"
              className="flex-1 px-md h-10 rounded-lg bg-surface-container-high border border-outline-variant/25 focus:border-primary/60 outline-none font-body-sm text-body-sm"
            />
            <button
              onClick={() => void lookup()}
              disabled={loading || !input.trim()}
              className="px-md rounded-lg bg-primary/12 text-primary font-label-md text-label-md hover:bg-primary/20 transition disabled:opacity-40"
            >
              {loading ? "查询中" : "查询"}
            </button>
          </div>

          {error ? (
            <div className="px-md py-sm rounded-lg bg-error/10 text-error font-body-sm text-body-sm">
              {error}
            </div>
          ) : null}

          {share ? (
            <div className="rounded-lg border border-outline-variant/25 overflow-hidden">
              {coverUrl && !imageFailed ? (
                <img
                  src={coverUrl}
                  alt={share.title}
                  className="w-full h-40 object-cover bg-surface-container-high"
                  onError={() => setImageFailed(true)}
                />
              ) : null}
              <div className="p-md space-y-sm">
                <div className="font-title-sm text-title-sm text-on-surface">{share.title}</div>
                <div className="text-[11px] text-on-surface-variant">
                  {share.contributorName ? `由 ${share.contributorName} 分享 · ` : ""}
                  {share.importCount} 次导入
                </div>
                {share.description ? (
                  <p className="text-body-sm text-on-surface-variant whitespace-pre-wrap">
                    {share.description}
                  </p>
                ) : null}
                <div className="grid grid-cols-3 gap-sm">
                  {[
                    ["亮度", share.color.brightness],
                    ["对比度", share.color.contrast],
                    ["Gamma", share.color.gamma],
                    ["数字振动", share.color.digitalVibrance],
                    ["RGB", `${share.color.rgbR}/${share.color.rgbG}/${share.color.rgbB}`],
                    ["分辨率", share.resolution || "不调整"],
                  ].map(([k, v]) => (
                    <div key={String(k)} className="px-sm py-2 rounded-lg bg-surface-container-high/60">
                      <div className="text-[11px] text-on-surface-variant">{k}</div>
                      <div className="font-label-md text-label-md text-on-surface">{v}</div>
                    </div>
                  ))}
                </div>
                <div className="flex items-center gap-xs text-[11px] text-on-surface-variant">
                  <Icon name={share.iccName ? "download" : "info"} className="text-[14px]" />
                  {share.iccName ? `含 ICC：${share.iccName}` : "只含色彩参数，不含 ICC 文件"}
                </div>
              </div>
            </div>
          ) : null}
        </div>

        <div className="shrink-0 px-lg py-md border-t border-outline-variant/15 flex items-center justify-between gap-sm">
          <span className="text-[11px] text-on-surface-variant">
            {stage || "导入不会覆盖你已有的同名方案"}
          </span>
          <div className="flex gap-sm">
            <button
              onClick={onClose}
              className="px-md py-sm rounded-md text-on-surface-variant font-label-md text-label-md hover:bg-surface-variant/50 transition"
            >
              取消
            </button>
            <button
              onClick={() => void doImport()}
              disabled={!share || importing}
              className="px-md py-sm rounded-md bg-primary text-on-primary font-label-md text-label-md hover:brightness-110 transition disabled:opacity-40"
            >
              {importing ? stage || "导入中…" : "导入这条方案"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
