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
    const cover = assetUrl(item.coverKey || item.thumbKey);
    const color = item.color || {};
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
      `<button type="button" class="fm-btn fm-btn-primary" id="fm-open-client">⚡ 在 Filter Manage 中打开</button>` +
      `<button type="button" class="fm-btn fm-btn-alt" id="fm-copy-link">复制链接</button>` +
      "</div></div>" +
      (cover
        ? `<div class="fm-share-cover"><img src="${esc(cover)}" alt="${esc(item.title)}"></div>`
        : "") +
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
