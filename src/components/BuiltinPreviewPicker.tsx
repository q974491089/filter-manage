import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  BUILTIN_SCENES, bytesBase64, hasPreviewIcc, isNeutralPreview, NEUTRAL_PREVIEW_SETTINGS,
  peekBuiltinPreview, renderBuiltinPreview, type BuiltinSceneId, type PreviewSettings,
} from "../lib/builtinPreview";
import { Icon } from "./Icon";
import SegmentedTabs from "./SegmentedTabs";

interface Props {
  scene: BuiltinSceneId;
  onChange: (scene: BuiltinSceneId) => void;
  settings?: PreviewSettings;
  iccBase64?: string | null;
  iccFile?: File | null;
  profileName?: string | null;
  disabled?: boolean;
}

export default function BuiltinPreviewPicker({ scene, onChange, settings = NEUTRAL_PREVIEW_SETTINGS, iccBase64, iccFile, profileName, disabled = false }: Props) {
  /** 每个场景的模拟效果图（data URL）；打开/参数变化时三张一起预生成，点 tab 零等待 */
  const [effects, setEffects] = useState<Partial<Record<BuiltinSceneId, string>>>({});
  const [original, setOriginal] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [zoomOpen, setZoomOpen] = useState(false);
  const sequence = useRef(0);
  const settingsKey = JSON.stringify(settings);
  const effect = effects[scene] ?? null;
  const originalSrc = `/game-previews/${scene}.png`;
  const shownSrc = original || !effect ? originalSrc : effect;

  useEffect(() => {
    const request = ++sequence.current;
    const settingsValue = JSON.parse(settingsKey) as PreviewSettings;
    setError("");

    // 参数全中性又没有 ICC 时结果就是原图本身，不必走一次渲染。
    if (!iccFile && !hasPreviewIcc(iccBase64, profileName) && isNeutralPreview(settingsValue)) {
      setEffects({});
      setBusy(false);
      return;
    }

    const baseOptions = {
      settings: settingsValue,
      iccBase64,
      profileName,
      // 用文件元信息当身份标识，命中缓存时连文件都不用读。
      iccToken: iccFile ? `file:${iccFile.name}:${iccFile.size}:${iccFile.lastModified}` : undefined,
    };
    // 当前场景已有缓存则立即显示，其余场景后台继续补齐
    const cachedNow = peekBuiltinPreview({ scene, ...baseOptions });
    setEffects(cachedNow ? { [scene]: cachedNow } : {});

    setBusy(true);
    const timer = window.setTimeout(() => {
      void (async () => {
        let encoded = iccBase64;
        if (iccFile) {
          if (iccFile.size > 2 * 1024 * 1024 || !/\.(icc|icm)$/i.test(iccFile.name)) throw new Error("请选择不超过 2 MB 的 ICC 文件");
          encoded = bytesBase64(await iccFile.arrayBuffer());
        }
        if (request !== sequence.current) return;
        const profileForRender = encoded ? null : profileName;
        // 三个场景一次全部生成（Rust 端并行渲染），点 tab 直接命中缓存零等待
        await Promise.allSettled(
          BUILTIN_SCENES.map(({ id }) =>
            renderBuiltinPreview({
              ...baseOptions,
              scene: id,
              iccBase64: encoded,
              profileName: profileForRender,
            }).then((data) => {
              if (request !== sequence.current) return;
              setEffects((prev) => ({ ...prev, [id]: data }));
              if (id === scene) setBusy(false);
            }).catch((err: unknown) => {
              if (request === sequence.current) {
                setError(err instanceof Error ? err.message : String(err));
                setBusy(false);
              }
            }),
          ),
        );
      })().catch((err: unknown) => {
        if (request === sequence.current) {
          setError(err instanceof Error ? err.message : String(err));
          setBusy(false);
        }
      });
    }, 200);
    return () => { window.clearTimeout(timer); sequence.current++; };
  }, [scene, settingsKey, iccBase64, iccFile, profileName]);

  // 放大预览打开时，Esc 只关闭放大层，不连带关闭外层弹窗（捕获阶段先拦截）。
  useEffect(() => {
    if (!zoomOpen) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      event.stopPropagation();
      setZoomOpen(false);
    };
    document.addEventListener("keydown", onKeyDown, true);
    return () => document.removeEventListener("keydown", onKeyDown, true);
  }, [zoomOpen]);

  const selected = BUILTIN_SCENES.find((item) => item.id === scene)!;
  return (
    <div data-component="BuiltinPreviewPicker" className="space-y-sm">
      <div className="overflow-hidden rounded-md border border-outline-variant/20 bg-surface-container-highest">
        <div className="relative aspect-video w-full overflow-hidden">
          {/* 场景切换：右上角纯文字 tab，替代原先占一整行的三缩略图 */}
          <SegmentedTabs
            items={BUILTIN_SCENES.map((item) => ({ id: item.id, label: item.game }))}
            value={scene}
            onChange={onChange}
            disabled={disabled}
            variant="overlay"
            ariaLabel="切换预览场景"
            className="absolute right-sm top-sm z-10"
          />
          <button type="button" onClick={() => setZoomOpen(true)} aria-label={`放大查看 ${selected.name} 预览图`}
            className="group relative block h-full w-full cursor-zoom-in focus-visible:outline focus-visible:outline-2 focus-visible:outline-primary">
            <img src={shownSrc} alt={`${selected.name} · ${original ? "原图" : "模拟效果"}`} className="h-full w-full object-contain" draggable={false} />
            <span className="pointer-events-none absolute bottom-sm right-sm flex items-center gap-xs rounded bg-black/60 px-sm py-xs text-[11px] text-white opacity-70 transition-opacity group-hover:opacity-100">
              <Icon name="zoom_in" className="text-[14px]" />
              点击放大
            </span>
          </button>
          <span className="pointer-events-none absolute left-sm top-sm rounded bg-black/70 px-sm py-xs text-[11px] text-white">{original ? "原图" : busy ? "正在生成预览…" : effect ? "模拟效果" : "内置原图"}</span>
        </div>
        <div className="flex items-center justify-between gap-sm px-sm py-xs">
          <span className="text-[11px] text-on-surface-variant">{selected.name} · 官方截图</span>
          <SegmentedTabs
            items={[
              { id: "original", label: "原图" },
              { id: "effect", label: "效果" },
            ]}
            value={original ? "original" : "effect"}
            onChange={(id) => setOriginal(id === "original")}
            variant="overlay"
            ariaLabel="预览对比"
            className="w-[124px]"
          />
        </div>
      </div>
      {error ? <p role="status" className="text-body-sm text-error">预览暂不可用：{error}</p> : null}
      <p className="text-[11px] leading-relaxed text-on-surface-variant">
        「效果」= 这套参数套在真实游戏画面上的样子，「原图」是未调色的画面，可以来回对比。
        分享后对方看到的预览和你选的场景一致；不同显示器和游戏里的实际观感会有差别。
        <button type="button" onClick={() => void openUrl(selected.source).catch(() => setError("无法打开图片来源"))} className="ml-1 text-primary hover:underline">图片来源</button>
      </p>

      {zoomOpen ? createPortal(
        <div data-component="BuiltinPreviewPicker" data-name="zoom"
          className="fixed inset-0 z-[400] flex flex-col items-center justify-center gap-md bg-black/90 p-4 sm:p-8"
          role="dialog" aria-modal="true" aria-label="预览图放大查看"
          onClick={() => setZoomOpen(false)}>
          <img src={shownSrc} alt={`${selected.name} · ${original ? "原图" : "模拟效果"}`} draggable={false}
            onClick={(event) => event.stopPropagation()}
            className="max-h-[82vh] max-w-[96vw] rounded-md object-contain shadow-2xl" />
          <div className="flex flex-wrap items-center justify-center gap-sm" onClick={(event) => event.stopPropagation()}>
            <span className="text-label-sm text-white/70">{selected.name} · {selected.game}</span>
            <div className="flex gap-xs" role="group" aria-label="放大预览对比">
              <button type="button" aria-pressed={original} onClick={() => setOriginal(true)}
                className={`rounded px-sm py-xs text-label-sm transition-colors ${original ? "bg-white/20 text-white" : "text-white/60 hover:text-white"}`}>原图</button>
              <button type="button" aria-pressed={!original} onClick={() => setOriginal(false)}
                className={`rounded px-sm py-xs text-label-sm transition-colors ${!original ? "bg-white/20 text-white" : "text-white/60 hover:text-white"}`}>效果</button>
            </div>
            <button type="button" onClick={() => setZoomOpen(false)} aria-label="关闭放大预览"
              className="grid h-8 w-8 place-items-center rounded-md text-white/70 transition-colors hover:bg-white/15 hover:text-white">
              <Icon name="close" className="text-[18px]" />
            </button>
          </div>
          <p className="text-[11px] text-white/45">点击空白处或按 Esc 关闭</p>
        </div>,
        document.body,
      ) : null}
    </div>
  );
}
