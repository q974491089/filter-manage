/**
 * 方案分享落地页渲染脚本（/s/<slug>）。
 *
 * 无框架单文件：从 filter-manage-admin 域的公开接口拉分享数据，渲染参数卡片、
 * ICC 下载与「在客户端中打开」。样式与客户端/管理端落地页同一套视觉（紫色
 * 系卡片），配色取自官网 VitePress 主题变量并带兜底值，亮暗色自适应。
 *
 * 数据契约见 filter-manage 仓库 .docs/api/share.md。接口是公开只读数据
 * （服务端对该前缀返回 Access-Control-Allow-Origin: *），可以跨域直读。
 */
(() => {
  "use strict";

  const API_BASE = "https://filter-manage-admin.xyls.us.kg";
  const ASSET_BASE = "https://filter-manage-download.xyls.us.kg";
  const SITE_BASE = "https://filter-manage.6ya.site";

  const root = document.getElementById("fm-share-root");
  if (!root) return;

  const slug = (() => {
    const m = location.pathname.match(/\/s\/([A-Za-z0-9_-]{10,64})/);
    return m ? m[1] : "";
  })();

  const esc = (s) =>
    String(s ?? "")
      .replaceAll("&", "&amp;")
      .replaceAll("<", "&lt;")
      .replaceAll(">", "&gt;")
      .replaceAll('"', "&quot;")
      .replaceAll("'", "&#39;");

  const assetUrl = (key) => (key ? `${ASSET_BASE}/${key}` : "");

  // ─── 内置预览：浏览器本地渲染模拟效果（与客户端 Rust 渲染同一套数学） ───
  //
  // 预览是纯函数：内置场景图 × 色彩参数 × ICC 曲线 → 像素。三张场景图是
  // /game-previews/<id>.jpg 静态资源，ICC 字节从分享详情的公开下载接口取，
  // 服务器不参与任何计算。数学逐行对照 src-tauri/src/nvidia.rs 的
  // calculate_lut / compose_gamma_ramp 与 icc.rs 的 parse_vcgt、
  // builtin_preview.rs 的 render，保证接收端（App/Web）与发送方预览一致。

  const SCENE_LABELS = {
    deltaforce: "室内外曝光 · 三角洲行动",
    cs2: "户外强光 · CS2",
    valorant: "霓虹色彩 · 瓦洛兰特",
  };
  /** 旧版分享保存过的场景 id → 现行场景 */
  const LEGACY_SCENES = { solarium: "cs2", siege: "deltaforce", capture: "valorant" };

  function normalizeScene(raw) {
    const id = LEGACY_SCENES[raw] ?? raw;
    return SCENE_LABELS[id] ? id : "deltaforce";
  }

  const clamp = (v, lo, hi) => Math.min(Math.max(v, lo), hi);
  const readU16BE = (d, off) => (d[off] << 8) | d[off + 1];
  const readU32BE = (d, off) =>
    ((d[off] << 24) | (d[off + 1] << 16) | (d[off + 2] << 8) | d[off + 3]) >>> 0;
  /** ICC s15Fixed16 定点数（raw i32 / 65536），符号还原用 |0 */
  const readS15Fixed16 = (d, off) => (readU32BE(d, off) | 0) / 65536;

  /** 移植 icc.rs parse_vcgt：解析 vcgt 标签 → [R,G,B] 各 256 级 u16 曲线；无效返回 null */
  function parseVcgt(data) {
    if (data.length < 132) return null;
    const tagCount = readU32BE(data, 128);
    let vcgtOffset = 0;
    let vcgtSize = 0;
    for (let i = 0; i < tagCount; i++) {
      const base = 132 + i * 12;
      if (base + 12 > data.length) break;
      if (readU32BE(data, base) === 0x76636774) {
        vcgtOffset = readU32BE(data, base + 4);
        vcgtSize = readU32BE(data, base + 8);
        break;
      }
    }
    if (vcgtOffset === 0 || vcgtOffset + 12 > data.length) return null;
    if (vcgtSize < 12 || vcgtOffset + vcgtSize > data.length) return null;
    const vcgt = data.subarray(vcgtOffset, vcgtOffset + vcgtSize);
    const gammaType = readU32BE(vcgt, 8);
    const ramp = [new Uint16Array(256), new Uint16Array(256), new Uint16Array(256)];
    if (gammaType === 0) {
      // 表格式：12 字节头 + 每通道 entryCount × entrySize
      if (vcgt.length < 18) return null;
      const channels = readU16BE(vcgt, 12);
      const entryCount = readU16BE(vcgt, 14);
      const entrySize = readU16BE(vcgt, 16);
      if (channels < 3 || entryCount === 0 || entrySize === 0) return null;
      if (entrySize !== 1 && entrySize !== 2) return null;
      if (vcgt.length < channels * entryCount * entrySize + 18) return null;
      for (let ch = 0; ch < 3; ch++) {
        for (let i = 0; i < 256; i++) {
          let srcIdx = Math.floor((i * (entryCount - 1)) / 255);
          if (srcIdx > entryCount - 1) srcIdx = entryCount - 1;
          const off = 18 + ch * entryCount * entrySize + srcIdx * entrySize;
          ramp[ch][i] = entrySize === 2 ? readU16BE(vcgt, off) : vcgt[off] << 8;
        }
      }
    } else if (gammaType === 1) {
      // 公式式：每通道 {gamma, min, max} 三个 s15Fixed16
      if (vcgt.length < 12 + 9 * 4) return null;
      for (let ch = 0; ch < 3; ch++) {
        const base = 12 + ch * 12;
        const gamma = readS15Fixed16(vcgt, base);
        const min = readS15Fixed16(vcgt, base + 4);
        const max = readS15Fixed16(vcgt, base + 8);
        if (!(gamma > 0) || max <= min) return null;
        for (let i = 0; i < 256; i++) {
          const y = clamp(min + (max - min) * Math.pow(i / 255, gamma), 0, 1);
          ramp[ch][i] = Math.round(y * 65535);
        }
      }
    } else {
      return null;
    }
    return ramp;
  }

  /** 移植 nvidia.rs calculate_lut：亮度/对比度线性映射 + 独立伽马，输出 0..65535 */
  function calculateLut(b, c, g) {
    const contrast = (clamp(c, 0, 1) - 0.5) * 2;
    const brightness = (clamp(b, 0, 1) - 0.5) * 2;
    let offset = contrast > 0 ? contrast * -25.4 : contrast * -32;
    const range = 255 + offset * 2;
    offset += brightness * (range / 5);
    const gamma = clamp(g, 0.4, 2.8);
    const lut = new Uint16Array(256);
    for (let i = 0; i < 256; i++) {
      const linear = clamp((i + offset) / range, 0, 1);
      const value = Math.abs(gamma - 1) < 1e-6 ? linear : Math.pow(linear, 1 / gamma);
      lut[i] = Math.round(value * 65535);
    }
    return lut;
  }

  const rgbGainToScale = (gain) => 1 + (clamp(gain, -100, 100) / 100) * 0.48;

  /** 移植 nvidia.rs compose_gamma_ramp：LUT 作索引叠加 ICC 曲线，再乘通道增益 */
  function composeGammaRamp(s, iccBase) {
    const b = (s.brightness + 125) / 250;
    const c = (s.contrast + 82) / 164;
    const lut = calculateLut(b, c, clamp(s.gamma, 0.4, 2.8));
    const scales = [rgbGainToScale(s.rgbR), rgbGainToScale(s.rgbG), rgbGainToScale(s.rgbB)];
    const ramp = new Uint16Array(768);
    for (let i = 0; i < 256; i++) {
      const idx = Math.min(Math.floor((lut[i] * 255) / 65535), 255);
      for (let ch = 0; ch < 3; ch++) {
        const base = iccBase ? iccBase[ch][idx] : lut[i];
        ramp[ch * 256 + i] = clamp(Math.round(base * scales[ch]), 0, 65535);
      }
    }
    return ramp;
  }

  /** 与 builtin_preview.rs render 同一套像素管线：ramp → 亮度混合 → 数字振动（饱和度） */
  async function renderBuiltinPreview(sceneId, color, iccCurve) {
    const res = await fetch(`/game-previews/${sceneId}.jpg`);
    if (!res.ok) throw new Error("scene image unavailable");
    const blob = await res.blob();
    // 无 ICC profile 的 JPEG 浏览器本就不做色彩转换；colorSpaceConversion:'none' 再上一道保险
    const bitmap = await createImageBitmap(blob, { colorSpaceConversion: "none" });
    const canvas = document.createElement("canvas");
    canvas.width = bitmap.width;
    canvas.height = bitmap.height;
    const ctx = canvas.getContext("2d", { willReadFrequently: true });
    ctx.drawImage(bitmap, 0, 0);
    const img = ctx.getImageData(0, 0, canvas.width, canvas.height);
    const ramp = composeGammaRamp(color, iccCurve);
    const saturation = clamp(color.digitalVibrance, 0, 100) / 50;
    const d = img.data;
    for (let i = 0; i < d.length; i += 4) {
      const r = ramp[d[i]] / 257;
      const g = ramp[256 + d[i + 1]] / 257;
      const b = ramp[512 + d[i + 2]] / 257;
      const lum = r * 0.2126 + g * 0.7152 + b * 0.0722;
      d[i] = clamp(Math.round(lum + (r - lum) * saturation), 0, 255);
      d[i + 1] = clamp(Math.round(lum + (g - lum) * saturation), 0, 255);
      d[i + 2] = clamp(Math.round(lum + (b - lum) * saturation), 0, 255);
    }
    ctx.putImageData(img, 0, 0);
    bitmap.close?.();
    return canvas.toDataURL("image/png");
  }

  const isNeutralColor = (c) =>
    (c?.brightness ?? 0) === 0 && (c?.contrast ?? 0) === 0 && (c?.gamma ?? 1) === 1 &&
    (c?.digitalVibrance ?? 50) === 50 && !(c?.rgbR || c?.rgbG || c?.rgbB);

  /** resolution 形如 2560x1440@144；空表示不调整 */
  const resolutionText = (r) => (r ? r : "不调整");

  const copyText = async (text) => {
    try {
      await navigator.clipboard.writeText(text);
      return true;
    } catch {
      return false;
    }
  };

  const toast = (msg, ok = true) => {
    const el = document.createElement("div");
    el.className = `fm-toast ${ok ? "ok" : "warn"}`;
    el.textContent = msg;
    document.body.appendChild(el);
    requestAnimationFrame(() => el.classList.add("show"));
    setTimeout(() => {
      el.classList.remove("show");
      setTimeout(() => el.remove(), 300);
    }, 2200);
  };

  function skeleton() {
    root.innerHTML =
      '<div class="fm-share"><div class="fm-share-card"><div class="fm-skel fm-skel-title"></div>' +
      '<div class="fm-skel fm-skel-line"></div><div class="fm-skel fm-skel-line short"></div>' +
      '<div class="fm-skel fm-skel-block"></div><div class="fm-skel fm-skel-block short"></div></div></div>';
  }

  function notFound() {
    root.innerHTML =
      '<div class="fm-share"><div class="fm-share-card fm-share-center">' +
      '<div class="fm-share-emoji">🔗</div>' +
      "<h2>分享不存在或已失效</h2>" +
      '<p class="fm-share-sub">链接可能已被分享者撤回，或地址不完整。</p>' +
      `<p><a class="fm-btn fm-btn-alt" href="${SITE_BASE}/">去官网了解 Filter Manage</a></p>` +
      "</div></div>";
  }

  function render(item) {
    const color = item.color || {};
    const sceneId = normalizeScene(item.previewScene);
    const cells = [
      ["亮度", color.brightness],
      ["对比度", color.contrast],
      ["Gamma", color.gamma],
      ["数字振动", color.digitalVibrance],
      [
        "RGB 增益",
        color.rgbR != null ? `${color.rgbR} / ${color.rgbG} / ${color.rgbB}` : "—",
      ],
      ["分辨率", resolutionText(item.resolution)],
    ];

    root.innerHTML =
      '<div class="fm-share"><div class="fm-share-card">' +
      '<div class="fm-share-head"><div>' +
      '<div class="fm-share-kicker">Filter Manage 方案分享</div>' +
      `<h1 class="fm-share-title">${esc(item.title)}</h1>` +
      '<div class="fm-share-meta">' +
      (item.contributorName ? `<span>由 ${esc(item.contributorName)} 分享</span>` : "") +
      `<span>${esc(new Date(item.createdAt).toLocaleDateString("zh-CN"))}</span>` +
      `<span>${esc(item.importCount)} 次导入</span>` +
      "</div></div>" +
      '<div class="fm-share-actions">' +
      `<button type="button" class="fm-btn fm-btn-primary" id="fm-open-client"><img class="fm-btn-icon" src="/favicon.png" alt="" width="20" height="20">在 Filter Manage 中打开</button>` +
      `<button type="button" class="fm-btn fm-btn-alt" id="fm-copy-link">复制链接</button>` +
      "</div></div>" +
      '<div class="fm-share-cover" id="fm-share-cover">' +
      `<img id="fm-preview-img" src="/game-previews/${sceneId}.jpg" alt="${esc(item.title)}" title="${esc(SCENE_LABELS[sceneId])}">` +
      '<span class="fm-preview-badge" id="fm-preview-badge">内置原图</span>' +
      '<div class="fm-preview-toggle" id="fm-preview-toggle" hidden>' +
      '<button type="button" data-mode="original">原图</button>' +
      '<button type="button" data-mode="effect" class="active">效果</button>' +
      "</div>" +
      "</div>" +
      (item.description
        ? `<p class="fm-share-desc">${esc(item.description)}</p>`
        : "") +
      '<div class="fm-share-section"><h3>色彩参数</h3><div class="fm-share-grid">' +
      cells
        .map(
          ([k, v]) =>
            `<div class="cell"><span class="k">${esc(k)}</span><span class="v">${esc(v)}</span></div>`,
        )
        .join("") +
      "</div></div>" +
      '<div class="fm-share-section"><h3>ICC 配置文件</h3>' +
      (item.iccName
        ? `<div class="fm-share-icc"><div><strong>${esc(item.iccName)}</strong>` +
          `<div class="fm-share-meta"><span>${esc((item.sizeBytes / 1024).toFixed(1))} KB</span>` +
          `<span class="mono">${esc(item.iccSha256.slice(0, 16))}…</span></div></div>` +
          `<a class="fm-btn fm-btn-alt" href="${esc(
            `${API_BASE}/api/market/shares/${encodeURIComponent(item.slug)}/icc`,
          )}" rel="noreferrer">下载</a></div>`
        : '<p class="fm-share-sub">这条方案只分享色彩参数，不包含 ICC 文件。</p>') +
      "</div>" +
      '<div class="fm-share-howto"><h3>怎么用</h3><ol>' +
      "<li>点上方「在 Filter Manage 中打开」，客户端会自动弹出导入窗</li>" +
      "<li>若未安装或浏览器提示无法打开，复制链接后在客户端点「导入分享链接」粘贴导入</li>" +
      "<li>确认后方案会加入你的方案列表，可一键应用</li>" +
      "</ol><p class=\"fm-share-sub\">首次使用协议打开会弹出系统管理员确认（UAC），属正常现象。" +
      "ICC 文件在导入时自动下载并校验格式；不含 ICC 的方案只调整色彩参数与分辨率。</p>" +
      '<p class="fm-share-hint" id="fm-client-hint" hidden></p>' +
      "</div>" +
      '<p class="fm-share-footer"><a href="https://github.com/q974491089/filter-manage" target="_blank" rel="noopener">Filter Manage</a>' +
      " · 免费开源的 Windows ICC / NVIDIA 颜色管理工具</p>" +
      "</div></div>";

    const openBtn = document.getElementById("fm-open-client");
    const hint = document.getElementById("fm-client-hint");
    openBtn?.addEventListener("click", () => {
      // 1.5s 没切走页面就提示复制导入（协议未注册 / 未安装客户端）
      const timer = setTimeout(() => {
        hint.hidden = false;
        hint.textContent =
          "未检测到 Filter Manage 客户端。请复制链接，在客户端中点「导入分享链接」粘贴导入。";
      }, 1500);
      window.addEventListener(
        "blur",
        () => {
          clearTimeout(timer);
          hint.hidden = true;
        },
        { once: true },
      );
      location.href = `filter-manage://import?slug=${encodeURIComponent(item.slug)}`;
    });

    document.getElementById("fm-copy-link")?.addEventListener("click", async () => {
      toast(
        (await copyText(location.href))
          ? "分享链接已复制"
          : "复制失败，请手动选中地址栏复制",
      );
    });

    void startPreview(item, sceneId);
  }

  /** 拉取分享 ICC → 本地渲染模拟效果 → 替换横幅并提供原图/效果切换 */
  async function startPreview(item, sceneId) {
    const imgEl = document.getElementById("fm-preview-img");
    if (!imgEl) return;
    const color = item.color || {};
    let iccCurve = null;
    if (item.iccSha256) {
      try {
        const res = await fetch(
          `${API_BASE}/api/market/shares/${encodeURIComponent(item.slug)}/icc`,
        );
        if (res.ok) iccCurve = parseVcgt(new Uint8Array(await res.arrayBuffer()));
      } catch {
        // 拉不到 ICC 就只用色彩参数渲染，不阻塞页面
      }
    }
    // 参数全中性又没有 ICC 曲线：效果与原图逐像素一致，不必渲染
    if (isNeutralColor(color) && !iccCurve) return;
    let effectUrl = "";
    try {
      effectUrl = await renderBuiltinPreview(sceneId, color, iccCurve);
    } catch {
      return; // 渲染不可用（老浏览器等）：保持内置原图展示
    }
    if (!document.getElementById("fm-preview-img")) return; // 用户已离开本页
    imgEl.src = effectUrl;
    const badge = document.getElementById("fm-preview-badge");
    if (badge) badge.textContent = "模拟效果";
    const toggle = document.getElementById("fm-preview-toggle");
    if (toggle) {
      toggle.hidden = false;
      toggle.addEventListener("click", (ev) => {
        const btn = ev.target.closest("button[data-mode]");
        if (!btn) return;
        for (const b of toggle.querySelectorAll("button")) {
          b.classList.toggle("active", b === btn);
        }
        imgEl.src =
          btn.dataset.mode === "original"
            ? `/game-previews/${sceneId}.jpg`
            : effectUrl;
        if (badge) {
          badge.textContent = btn.dataset.mode === "original" ? "内置原图" : "模拟效果";
        }
      });
    }
  }

  async function load() {
    if (!slug) {
      notFound();
      return;
    }
    skeleton();
    try {
      const res = await fetch(
        `${API_BASE}/api/market/shares/${encodeURIComponent(slug)}`,
        { headers: { Accept: "application/json" } },
      );
      if (!res.ok) {
        notFound();
        return;
      }
      const data = await res.json();
      if (data?.item) render(data.item);
      else notFound();
    } catch {
      notFound();
    }
  }

  void load();
})();
