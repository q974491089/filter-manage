<template>
  <div class="hero-actions">
    <!-- 主按钮：立即下载（带动画） -->
    <a :href="downloadUrl" class="hero-btn hero-btn-primary" @click.prevent="handleDownload">
      <span class="btn-text">立即下载</span>
      <span class="btn-suffix">免费 v{{ version }}</span>
    </a>

    <!-- 次要按钮 -->
    <div class="hero-btn-secondary">
      <a href="/guide/install.html#下载" class="hero-btn hero-btn-alt">
        备用下载
      </a>
      <a href="/guide/" class="hero-btn hero-btn-alt">
        使用指南
      </a>
      <a href="https://github.com/q974491089/filter-manage" class="hero-btn hero-btn-alt" target="_blank" rel="noopener">
        GitHub
      </a>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'

const version = ref('')
const downloadUrl = ref('#download')

onMounted(async () => {
  const hosts = [
    'https://filter-manage-api.xyls.us.kg',
    'https://filter-manage-api.6ya.kdns.fr',
  ]
  const controllers = hosts.map(() => new AbortController())

  const raceResult = await Promise.any(
    hosts.map(async (host, i) => {
      const res = await fetch(
        `${host}/api/check-update?current=0.0.0`,
        { signal: controllers[i].signal },
      )
      const data = await res.json()
      if (!data?.version) throw new Error('invalid response')
      for (let j = 0; j < controllers.length; j++) {
        if (j !== i) controllers[j].abort()
      }
      return data
    }),
  ).catch(() => null)

  if (raceResult) {
    const v = raceResult.version
    const mirrors = raceResult.mirrors
    version.value = v
    const ghPath = `https://github.com/q974491089/filter-manage/releases/download/v${v}/Filter-Manage_${v}_x64-setup.exe`
    const allMirrors = [
      ...(mirrors?.map(m => m.url) || []),
      `https://gh-proxy.com/${ghPath}`,
      `https://ghproxy.net/${ghPath}`,
      `https://ghfast.top/${ghPath}`,
      `https://gh.ddlc.top/${ghPath}`,
      `https://slink.ltd/${ghPath}`,
      ghPath,
    ]
    downloadUrl.value = allMirrors[0]
  }
})

function handleDownload() {
  window.location.href = downloadUrl.value
}
</script>

<style scoped>
.hero-actions {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 12px;
  padding-top: 24px;
  max-width: 480px;
}

.hero-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: 7px;
  font-weight: 500;
  text-decoration: none;
  cursor: pointer;
  font-family: inherit;
}

/* ─── 主按钮（带动画） ─── */
.hero-btn-primary {
  width: 100%;
  height: 46px;
  padding: 0 24px;
  font-size: 14px;
  background: var(--vp-c-brand-1);
  color: #fff;
  border: none;
  position: relative;
  box-shadow: 0px 6px 16px -4px var(--vp-c-brand-1);
}

.hero-btn-primary .btn-text {
  transition: 0.4s;
}

.hero-btn-primary .btn-suffix {
  opacity: 0;
  max-width: 0;
  overflow: hidden;
  transition: 0.4s;
  white-space: nowrap;
}

.hero-btn-primary:hover .btn-text {
  padding-right: 8px;
}

.hero-btn-primary:hover .btn-suffix {
  opacity: 1;
  max-width: 120px;
}

.hero-btn-primary:hover {
  box-shadow: 0px 12px 40px -6px var(--vp-c-brand-1);
}

/* ─── 次要按钮容器 ─── */
.hero-btn-secondary {
  display: flex;
  gap: 10px;
  width: 100%;
}

/* ─── 次要按钮 ─── */
.hero-btn-alt {
  flex: 1;
  height: 42px;
  padding: 0 16px;
  font-size: 14px;
  background: var(--vp-button-alt-bg);
  color: var(--vp-button-alt-color);
  border: 1px solid var(--vp-button-alt-border);
  transition: all 0.2s ease;
}

.hero-btn-alt:hover {
  background: var(--vp-button-alt-hover-bg);
  color: var(--vp-button-alt-hover-color);
}

/* ─── 暗色模式适配 ─── */
html.dark .hero-btn-primary {
  background: var(--vp-c-brand-1);
  color: var(--vp-c-white);
  box-shadow: 0px 6px 16px -4px rgba(0, 0, 0, 0.4);
}

html.dark .hero-btn-primary:hover {
  box-shadow: 0px 12px 40px -6px var(--vp-c-brand-1);
}

html.dark .hero-btn-alt {
  background: transparent;
  color: var(--vp-c-text-1);
  border-color: var(--vp-c-border);
}

html.dark .hero-btn-alt:hover {
  background: var(--vp-c-bg-soft);
  border-color: var(--vp-c-brand-1);
  color: var(--vp-c-brand-1);
}
</style>
