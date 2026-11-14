import { sha256 } from "@noble/hashes/sha2.js";
import { bytesToHex } from "@noble/hashes/utils.js";
import { invoke } from "./ipc";

export const BUILTIN_SCENES = [
  { id: "deltaforce", name: "室内外曝光", game: "三角洲行动", source: "https://store.steampowered.com/app/2507950/" },
  { id: "cs2", name: "户外强光", game: "CS2", source: "https://store.steampowered.com/app/730/" },
  { id: "valorant", name: "霓虹色彩", game: "瓦洛兰特", source: "https://store.epicgames.com/p/valorant" },
] as const;

export type BuiltinSceneId = typeof BUILTIN_SCENES[number]["id"];
export const DEFAULT_PREVIEW_SCENE: BuiltinSceneId = "deltaforce";

/** 旧版本分享/投稿里保存的场景 id，按画面对应到新场景。 */
const LEGACY_SCENES: Record<string, BuiltinSceneId> = {
  solarium: "cs2",
  siege: "deltaforce",
  capture: "valorant",
};

export interface PreviewSettings {
  brightness: number;
  contrast: number;
  gamma: number;
  digitalVibrance: number;
  rgbR: number;
  rgbG: number;
  rgbB: number;
}

export const NEUTRAL_PREVIEW_SETTINGS: PreviewSettings = {
  brightness: 0, contrast: 0, gamma: 1, digitalVibrance: 50, rgbR: 0, rgbG: 0, rgbB: 0,
};

export interface BuiltinPreviewOptions {
  scene: BuiltinSceneId;
  settings: PreviewSettings;
  iccBase64?: string | null;
  profileName?: string | null;
  /** 调用方已知的 ICC 身份（如 File 的 name/size/lastModified）；提供后不读文件内容即可命中缓存。 */
  iccToken?: string | null;
}

export function builtinScene(raw: string): BuiltinSceneId {
  const id = LEGACY_SCENES[raw] ?? raw;
  return BUILTIN_SCENES.find((scene) => scene.id === id)?.id ?? DEFAULT_PREVIEW_SCENE;
}

export function bytesBase64(buffer: ArrayBuffer): string {
  const bytes = new Uint8Array(buffer);
  let binary = "";
  for (let offset = 0; offset < bytes.length; offset += 0x8000) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + 0x8000));
  }
  return btoa(binary);
}

/** 参数全中性且没有 ICC 时，后端渲染结果与原图逐像素一致，直接复用静态原图。 */
export function isNeutralPreview(settings: PreviewSettings): boolean {
  return settings.brightness === 0 && settings.contrast === 0 && settings.gamma === 1
    && settings.digitalVibrance === 50 && settings.rgbR === 0 && settings.rgbG === 0 && settings.rgbB === 0;
}

/** 是否真的有 ICC 曲线要叠加（"Default" 与空值等价于不叠加）。 */
export function hasPreviewIcc(iccBase64?: string | null, profileName?: string | null): boolean {
  if (iccBase64) return true;
  const name = profileName?.trim();
  return Boolean(name && name !== "Default" && name !== "Default.icc");
}

/**
 * 预览缓存：同一「场景 + 参数 + ICC」只渲染一次。
 * 来回切换场景、切换原图/效果、关闭再打开弹窗时都直接复用，避免重复走 IPC 与像素重算。
 */
const PREVIEW_CACHE_LIMIT = 6;
const previewCache = new Map<string, string>();
const previewPending = new Map<string, Promise<string>>();
let lastIccSource: string | null = null;
let lastIccToken = "none";

function iccToken(iccBase64?: string | null, profileName?: string | null): string {
  if (iccBase64) {
    if (iccBase64 === lastIccSource) return lastIccToken;
    lastIccToken = `icc:${bytesToHex(sha256(new TextEncoder().encode(iccBase64))).slice(0, 24)}`;
    lastIccSource = iccBase64;
    return lastIccToken;
  }
  return hasPreviewIcc(null, profileName) ? `profile:${profileName!.trim()}` : "none";
}

export function previewCacheKey(options: BuiltinPreviewOptions): string {
  const token = options.iccToken?.trim() || iccToken(options.iccBase64, options.profileName);
  return `${options.scene}|${token}|${JSON.stringify(options.settings)}`;
}

/** 命中缓存时立即返回 data URL，未命中返回 null（不会触发渲染）。 */
export function peekBuiltinPreview(options: BuiltinPreviewOptions): string | null {
  const key = previewCacheKey(options);
  const value = previewCache.get(key);
  if (value === undefined) return null;
  // LRU：命中后移到队尾，淘汰最久未用的条目。
  previewCache.delete(key);
  previewCache.set(key, value);
  return value;
}

function rememberPreview(key: string, dataUrl: string) {
  previewCache.delete(key);
  previewCache.set(key, dataUrl);
  while (previewCache.size > PREVIEW_CACHE_LIMIT) {
    const oldest = previewCache.keys().next().value;
    if (oldest === undefined) break;
    previewCache.delete(oldest);
  }
}

export async function renderBuiltinPreview(options: BuiltinPreviewOptions): Promise<string> {
  const key = previewCacheKey(options);
  const cached = previewCache.get(key);
  if (cached !== undefined) {
    rememberPreview(key, cached);
    return cached;
  }
  const pending = previewPending.get(key);
  if (pending) return pending;

  const task = invoke<string>("render_builtin_icc_preview", {
    sceneId: options.scene,
    settings: options.settings,
    iccBase64: options.iccBase64 || null,
    profileName: options.profileName || null,
  })
    .then((base64) => {
      rememberPreview(key, `data:image/png;base64,${base64}`);
      return `data:image/png;base64,${base64}`;
    })
    .finally(() => {
      previewPending.delete(key);
    });
  previewPending.set(key, task);
  return task;
}
