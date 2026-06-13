<template>
  <Teleport to="body">
    <Transition name="slide">
      <div class="site-notice" v-if="visible && isClient">
        <div class="notice-header">
          <span class="notice-title">镜像站点</span>
          <button class="close-btn" @click="close" aria-label="关闭">×</button>
        </div>
        <div class="notice-body">
          <p class="notice-desc">如访问缓慢可尝试切换</p>
          <div class="site-links">
            <a 
              v-for="site in sites" 
              :key="site.key"
              :href="site.url" 
              :class="['site-link', { active: currentSite === site.key }]"
            >
              <span class="site-name">{{ site.name }}</span>
              <span class="site-check" v-if="currentSite === site.key">✓</span>
            </a>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup>
import { ref, onMounted } from 'vue'

const visible = ref(false)
const currentSite = ref('')
const isClient = ref(false)

const sites = [
  { key: '6ya.site', name: '主站', url: 'https://filter-manage.6ya.site/' },
  { key: 'vercel.app', name: '镜像 1', url: 'https://filter-manage.vercel.app/' },
  { key: 'xy18600.ggff.net', name: '镜像 2', url: 'https://filter-manage.xy18600.ggff.net/' },
  { key: 'xyls.us.kg', name: '镜像 3', url: 'https://filter-manage.xyls.us.kg/' },
]

onMounted(() => {
  isClient.value = true
  
  const host = window.location.hostname
  
  if (host.includes('6ya.site')) currentSite.value = '6ya.site'
  else if (host.includes('xy18600.ggff.net')) currentSite.value = 'xy18600.ggff.net'
  else if (host.includes('vercel.app')) currentSite.value = 'vercel.app'
  else if (host.includes('xyls.us.kg')) currentSite.value = 'xyls.us.kg'
  else currentSite.value = host

  // 检查是否已关闭过（24小时后重新显示）
  const closedTime = localStorage.getItem('site-notice-closed-time')
  if (closedTime) {
    const hoursPassed = (Date.now() - parseInt(closedTime)) / (1000 * 60 * 60)
    if (hoursPassed < 24) return
  }
  visible.value = true
})

function close() {
  visible.value = false
  localStorage.setItem('site-notice-closed-time', Date.now().toString())
}

defineExpose({ currentSite, sites })
</script>

<style scoped>
.site-notice {
  position: fixed;
  top: 80px;
  right: 20px;
  width: 240px;
  background: var(--vp-c-bg);
  border-radius: 8px;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.08);
  z-index: 1000;
  overflow: hidden;
  border: 1px solid var(--vp-c-divider);
}

.notice-header {
  display: flex;
  align-items: center;
  padding: 10px 14px;
  border-bottom: 1px solid var(--vp-c-divider);
}

.notice-title {
  flex: 1;
  font-size: 13px;
  font-weight: 600;
  color: var(--vp-c-text-1);
}

.close-btn {
  background: none;
  border: none;
  color: var(--vp-c-text-3);
  font-size: 16px;
  cursor: pointer;
  padding: 0;
  line-height: 1;
  transition: color 0.2s ease;
}

.close-btn:hover {
  color: var(--vp-c-text-1);
}

.notice-body {
  padding: 10px 14px 12px;
}

.notice-desc {
  margin: 0 0 10px 0;
  font-size: 12px;
  color: var(--vp-c-text-3);
}

.site-links {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.site-link {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 5px 10px;
  color: var(--vp-c-text-2);
  text-decoration: none;
  font-size: 12px;
  font-weight: 500;
  border-radius: 6px;
  border: 1px solid var(--vp-c-divider);
  transition: all 0.15s ease;
  background: var(--vp-c-bg-soft);
}

.site-link:hover {
  border-color: var(--vp-c-brand-1);
  color: var(--vp-c-brand-1);
}

.site-link.active {
  border-color: var(--vp-c-brand-1);
  background: var(--vp-c-brand-soft);
  color: var(--vp-c-brand-1);
}

.site-check {
  font-size: 10px;
}

/* 动画 */
.slide-enter-active {
  animation: slideIn 0.2s ease;
}

.slide-leave-active {
  animation: slideOut 0.15s ease;
}

@keyframes slideIn {
  from {
    transform: translateX(20px);
    opacity: 0;
  }
  to {
    transform: translateX(0);
    opacity: 1;
  }
}

@keyframes slideOut {
  from {
    transform: translateX(0);
    opacity: 1;
  }
  to {
    transform: translateX(20px);
    opacity: 0;
  }
}

@media (max-width: 768px) {
  .site-notice {
    top: auto;
    bottom: 20px;
    right: 16px;
    left: 16px;
    width: auto;
  }
}
</style>
