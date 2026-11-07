import { useEffect, useState } from "react";
import { invoke } from "../lib/ipc";
import { debugNote } from "../lib/debug";
import { sha256Hex } from "../lib/submissions";
import { serverEnv } from "../lib/serverEnv";
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
  /** 启动检测到剪贴板里的分享链接时预填，并自动查询 */
  initialSlug?: string | null;
  /** 导入后打算应用到的显示器（当前选中的显示器）；缺省按主显示器校验分辨率 */
  deviceId?: string;
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

/** 对应后端 icc::DisplayModeList / DisplayModeInfo / CurrentDisplayMode */
interface DisplayModeInfo {
  width: number;
  height: number;
  refresh_rates: number[];
  is_current: boolean;
}

interface DisplayModeList {
  modes: DisplayModeInfo[];
  current: { width: number; height: number; refresh_rate: number } | null;
}

/** 解析方案分辨率字段 "宽x高[@Hz]"（与后端 parse_resolution 同规则，容忍大写 X 与 ×） */
function parseResolutionSpec(spec: string): { w: number; h: number; hz?: number } | null {
  const m = spec.trim().match(/^(\d+)\s*[xX×]\s*(\d+)(?:\s*@\s*(\d+))?$/);
  if (!m) return null;
  const w = Number(m[1]);
  const h = Number(m[2]);
  const hz = m[3] !== undefined ? Number(m[3]) : undefined;
  if (w <= 0 || h <= 0) return null;
  if (hz !== undefined && hz <= 0) return null;
  return { w, h, hz };
}

/**
 * 导入预检：分享方案的分辨率当前显示器是否支持。返回落库值与给用户的说明。
 * - 宽高不支持 / 字段无效 → 丢弃整个字段（应用时保持用户当前分辨率）
 * - 宽高支持但指定刷新率不支持 → 只去掉 @Hz（沿用该分辨率的默认刷新率）
 * - 当前模式即使枚举缺失也算支持（与后端 preset_mode_supported 同口径，防驱动枚举抖动）
 */
function analyzeSharedResolution(spec: string, list: DisplayModeList): { value: string | null; note: string | null } {
  const parsed = parseResolutionSpec(spec);
  if (!parsed) {
    return { value: null, note: "方案中的分辨率字段无效，导入后将不携带分辨率" };
  }
  const matchesCurrent = list.current !== null
    && list.current.width === parsed.w
    && list.current.height === parsed.h
    && (parsed.hz === undefined || parsed.hz === list.current.refresh_rate);
  const mode = list.modes.find((m) => m.width === parsed.w && m.height === parsed.h);
  if (!matchesCurrent && !mode) {
    return {
      value: null,
      note: `当前显示器不支持 ${parsed.w}×${parsed.h}，已删除该分辨率；导入后应用时保持你现在的分辨率`,
    };
  }
  if (parsed.hz !== undefined && mode && !mode.refresh_rates.includes(parsed.hz) && !matchesCurrent) {
    return {
      value: `${parsed.w}x${parsed.h}`,
      note: `当前显示器不支持 ${parsed.w}×${parsed.h}@${parsed.hz}Hz，已删除失效的刷新率，导入后按该分辨率的默认刷新率应用`,
    };
  }
  return { value: spec, note: null };
}

const inputClass =
  "w-full h-9 px-sm rounded-md bg-surface-container-high border border-outline-variant/25 outline-none focus:border-primary/60 focus-visible:ring-2 focus-visible:ring-primary/30 text-body-sm text-on-surface transition-colors";

export default function ImportShareModal({
  open,
  onClose,
  showToast,
  onImported,
  initialSlug,
  deviceId,
}: ImportShareModalProps) {
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);
  const [share, setShare] = useState<PublicShare | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [importing, setImporting] = useState(false);
  const [stage, setStage] = useState("");
  const [imageFailed, setImageFailed] = useState(false);
  const [zoomOpen, setZoomOpen] = useState(false);
  /** 分辨率导入预检结果（null = 未预检或方案不带分辨率）；仅用于弹窗内提示，落库以 doImport 里的实时复检为准 */
  const [resCompat, setResCompat] = useState<{ value: string | null; note: string | null } | null>(null);
  const modalRef = useModalAccessibility(open, onClose, true, !importing);

  useEffect(() => {
    if (!open) return;
    setInput(initialSlug ?? "");
    setShare(null);
    setError(null);
    setStage("");
    setImageFailed(false);
    setResCompat(null);
  }, [open, initialSlug]);

  // 带着剪贴板 slug 打开时自动查询一次，用户只需确认导入。
  // 必须挂在 early return 之前，否则开合会改变 hook 数量直接崩掉组件。
  useEffect(() => {
    if (!open || !initialSlug) return;
    void lookup(initialSlug);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, initialSlug]);

  if (!open) return null;

  async function lookup(raw: string) {
    const slug = parseShareSlug(raw);
    if (!slug) {
      setError("这不像一条分享链接，请粘贴完整地址或分享码");
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const item = await getShare(slug);
      if (!item) {
        // 404 时点明当前接口环境：分享是环境作用域的，预发创建的在线上查不到
        const envLabel = serverEnv().apiEnv === "staging" ? "预发" : "线上";
        setError(`分享不存在或已被撤销（当前接口环境：${envLabel}）`);
        return;
      }
      setShare(item);
      setImageFailed(false);
      setResCompat(null);
      debugNote(`查看分享 ${slug}`);
      // 查到方案就立刻预检分辨率：用户在点导入之前就能看到"哪个分辨率会被删"
      if (item.resolution) {
        void analyzeResolutionCompat(item.resolution);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }

  /** 弹窗内的分辨率预检：枚举目标显示器支持的模式，标出会被删除的失效分辨率 */
  async function analyzeResolutionCompat(spec: string) {
    try {
      const list = await invoke<DisplayModeList>("get_display_modes", {
        deviceId: deviceId ?? null,
      });
      setResCompat(analyzeSharedResolution(spec, list));
    } catch (err) {
      // 枚举失败不拦导入：落库保留原值，应用时还有 apply_preset_resolution 兼容兜底
      debugNote(`导入预检分辨率失败（保留原值）：${String(err)}`);
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
        // 先看本机是不是已经有同一份文件（按内容 sha 对比），有就直接复用，
        // 没必要再从服务器拉一遍
        setStage("检查本地配置文件");
        let localSha: string | null = null;
        try {
          const file = await invoke<{ base64: string }>("read_icc_file", {
            name: share.iccName,
          });
          const binary = atob(file.base64);
          const bytes = new Uint8Array(binary.length);
          for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
          localSha = await sha256Hex(bytes.buffer);
        } catch {
          // 本机没有这个文件（或读取失败），走下载
        }

        if (localSha && localSha === share.iccSha256) {
          iccProfile = share.iccName;
          debugNote(`本机已有相同 ICC「${share.iccName}」，跳过下载`);
        } else {
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
      }

      setStage("保存方案");
      const name = await uniqueName(share.title);

      // 落库前实时复检（预览时的预检可能已过期：显示器/模式会变），
      // 不支持的分辨率在这里剔除，应用时保持用户当前分辨率
      let resolution: string | null = share.resolution || null;
      let resNote: string | null = null;
      if (resolution) {
        try {
          const list = await invoke<DisplayModeList>("get_display_modes", {
            deviceId: deviceId ?? null,
          });
          const compat = analyzeSharedResolution(resolution, list);
          resolution = compat.value;
          resNote = compat.note;
        } catch (err) {
          debugNote(`导入前分辨率复检失败（保留原值）：${String(err)}`);
        }
      }

      const config: ColorConfig = {
        name,
        brightness: share.color.brightness,
        contrast: share.color.contrast,
        gamma: share.color.gamma,
        digital_vibrance: share.color.digitalVibrance,
        rgb_r: share.color.rgbR,
        rgb_g: share.color.rgbG,
        rgb_b: share.color.rgbB,
        resolution,
        icc_profile: iccProfile,
      };
      await invoke("save_config", { config });
      void reportShareImport(share.slug);
      debugNote(`导入分享方案「${name}」${resNote ? `（${resNote}）` : ""}`);
      showToast("success", resNote ? `已导入「${name}」；${resNote}` : `已导入「${name}」`);
      onImported(name);
      onClose();
    } catch (err) {
      showToast("error", err instanceof Error ? err.message : String(err));
    } finally {
      setImporting(false);
      setStage("");
    }
  }

  // 大横幅用 1600px 原图（thumb 只有 480px，拉伸到 banner 宽度会糊）
  const coverUrl = share ? shareAssetUrl(share.coverKey || share.thumbKey) : "";

  const summaryPairs: Array<[string, string]> = share
    ? [
        ["亮度", String(share.color.brightness)],
        ["对比度", String(share.color.contrast)],
        ["Gamma", String(share.color.gamma)],
        ["数字振动", String(share.color.digitalVibrance)],
        ["RGB", `${share.color.rgbR} / ${share.color.rgbG} / ${share.color.rgbB}`],
        [
          "分辨率",
          share.resolution
            ? resCompat && resCompat.value === null
              ? `${share.resolution}（将移除）`
              : share.resolution
            : "不调整",
        ],
      ]
    : [];

  return (
    <div className="fixed inset-0 z-[300] flex items-center justify-center bg-black/60 p-3 backdrop-blur-[2px] sm:p-5">
      <div
        ref={modalRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="import-share-title"
        tabIndex={-1}
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
                if (e.key === "Enter") void lookup(input);
              }}
              placeholder="https://.../s/xxxxxxxx"
              aria-label="分享链接"
              className={inputClass}
            />
            <button
              onClick={() => void lookup(input)}
              disabled={loading || !input.trim()}
              className="min-h-9 shrink-0 px-lg rounded-md border border-primary/40 text-primary font-label-md text-label-md hover:bg-primary/20 transition-colors disabled:opacity-40 focus-visible:outline focus-visible:outline-2 focus-visible:outline-primary"
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
                <div className="group relative h-44 w-full overflow-hidden bg-surface-container-highest">
                  <img
                    src={coverUrl}
                    alt={share.title}
                    title="点击放大查看"
                    className="h-full w-full cursor-zoom-in object-cover transition-transform duration-500 ease-out group-hover:scale-[1.02] motion-reduce:transition-none motion-reduce:group-hover:scale-100"
                    onClick={() => setZoomOpen(true)}
                    onError={() => setImageFailed(true)}
                  />
                  {/* 浅灰边缘蒙层：柔化图片边界，与卡片底色过渡 */}
                  <div
                    aria-hidden
                    className="pointer-events-none absolute inset-0"
                    style={{
                      background:
                        "radial-gradient(ellipse at center, transparent 50%, rgba(205,205,214,0.32) 100%)",
                    }}
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
                {resCompat?.note ? (
                  <div
                    role="status"
                    className="flex items-start gap-xs rounded-md bg-tertiary/10 px-md py-sm text-[11px] leading-relaxed text-tertiary"
                  >
                    <Icon name="info" className="mt-[2px] shrink-0 text-[14px]" />
                    <span>{resCompat.note}</span>
                  </div>
                ) : null}
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
          <span className="mr-auto text-[11px] text-on-surface-variant">{stage}</span>
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

      {/* 图片放大查看：全屏遮罩，点任意处关闭 */}
      {zoomOpen && coverUrl ? (
        <div
          className="fixed inset-0 z-[400] flex items-center justify-center bg-black/90 p-6 cursor-zoom-out animate-[slide-fade-up_0.2s_cubic-bezier(0.16,1,0.3,1)_forwards] motion-reduce:animate-none"
          onClick={() => setZoomOpen(false)}
          role="dialog"
          aria-label="效果图放大查看"
        >
          <img
            src={coverUrl}
            alt={share?.title ?? "效果图"}
            className="max-h-[88vh] max-w-[94vw] rounded-md object-contain shadow-2xl"
          />
        </div>
      ) : null}
    </div>
  );
}
