// 方案分享落地页（EdgeOne Pages Edge Function）。
//
// 路由：edge-functions/s/[[default]].js → filter-manage.6ya.site/s/<slug>
// 职责：从 filter-manage-admin 域的公开接口拉取分享数据，在服务端注入
// <title> 与 og:* 预览（微信/QQ/论坛抓取看到的是方案内容而不是 SPA 骨架），
// 然后返回静态壳页面（/s/index.html，由 VitePress 构建产出），由壳页面里的
// 脚本完成实际渲染。接口是公开只读数据（marketCORS 允许任意来源），壳页面
// 可以直接跨域读取；这里不代理接口、不缓存数据。
//
// 数据契约见 filter-manage 仓库 .docs/api/share.md。函数仅注入元数据，分享
// 被撤销或 slug 不存在时也照常返回壳页面，由前端统一显示「链接已失效」。

const API_BASE = "https://filter-manage-admin.xyls.us.kg";
const SITE_BASE = "https://filter-manage.6ya.site";

/** 转义 HTML 特殊字符，防止方案标题/昵称里的字符逃出标签或注入属性。 */
function escapeHtml(text) {
  return String(text ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

function assetUrl(key) {
  return key ? `https://filter-manage-download.xyls.us.kg/${key}` : "";
}

/**
 * 从公开详情接口取分享元数据。任何失败（网络/404/非 JSON）都返回 null，
 * 函数照常出壳页面，不让一个坏 slug 拖垮整条路由。
 */
async function fetchShare(slug) {
  try {
    const res = await fetch(
      `${API_BASE}/api/market/shares/${encodeURIComponent(slug)}`,
      { headers: { Accept: "application/json" } },
    );
    if (!res.ok) return null;
    const data = await res.json();
    return data?.item ?? null;
  } catch {
    return null;
  }
}

/** slug 是 16 字节 base64url（22 字符），先挡掉明显非法的路径再打接口。 */
function isValidSlug(slug) {
  return /^[A-Za-z0-9_-]{10,64}$/.test(slug);
}

async function onRequestGet(context) {
  const slug = (context.params?.default ?? "").replace(/\/+$/, "");
  const share = isValidSlug(slug) ? await fetchShare(slug) : null;

  const title = share
    ? `${share.title} · Filter Manage 方案分享`
    : "Filter Manage 方案分享";
  const description = share
    ? (share.description || "").slice(0, 160) ||
      `${share.contributorName ? `由 ${share.contributorName} 分享，` : ""}一键导入这套 NVIDIA 调色方案` +
        (share.iccName ? `，含 ICC 配置文件 ${share.iccName}` : "")
    : "一键导入别人分享的 NVIDIA 调色方案：色彩参数 + 分辨率 + ICC 配置文件。";
  const ogImage = share ? assetUrl(share.coverKey || share.thumbKey) : `${SITE_BASE}/favicon.png`;

  const head = [
    `<title>${escapeHtml(title)}</title>`,
    `<meta name="description" content="${escapeHtml(description)}">`,
    `<meta property="og:type" content="website">`,
    `<meta property="og:title" content="${escapeHtml(title)}">`,
    `<meta property="og:description" content="${escapeHtml(description)}">`,
    `<meta property="og:url" content="${SITE_BASE}/s/${encodeURIComponent(slug)}">`,
    ogImage ? `<meta property="og:image" content="${escapeHtml(ogImage)}">` : "",
    `<meta name="twitter:card" content="summary_large_image">`,
  ]
    .filter(Boolean)
    .join("\n    ");

  const res = await context.env.ASSETS.fetch(
    new URL(`/s/index.html`, context.request.url),
  );
  const html = (await res.text()).replace("<!--edgeone-share-meta-->", head);

  return new Response(html, {
    status: 200,
    headers: {
      "content-type": "text/html; charset=utf-8",
      // 分享内容可变（撤销后要立刻失效），不做边缘缓存
      "cache-control": "no-store",
    },
  });
}

export { onRequestGet };
