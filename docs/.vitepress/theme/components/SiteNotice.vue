<template>
  <Teleport to="body">
    <Transition name="notification">
      <div class="notification-container" v-if="visible && isClient">
        <div class="notification">
          <div class="notification-content">
            <div class="notification-icon">
              <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
                <path d="M7 1C3.68629 1 1 3.68629 1 7C1 10.3137 3.68629 13 7 13C10.3137 13 13 10.3137 13 7C13 3.68629 10.3137 1 7 1Z" stroke="currentColor" stroke-width="1.2"/>
                <path d="M7 4V7.5L9 9.5" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>
              </svg>
            </div>
            <div class="notification-body">
              <div class="notification-title">镜像站点</div>
              <div class="notification-desc">如访问缓慢可尝试切换</div>
              <div class="site-links">
                <a 
                  v-for="site in sites" 
                  :key="site.key"
                  :href="site.url" 
                  :class="['site-link', { active: currentSite === site.key }]"
                >
                  {{ site.name }}
                  <span class="site-check" v-if="currentSite === site.key">✓</span>
                </a>
              </div>
            </div>
          </div>
          <button class="notification-close" @click="close">
            <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
              <path d="M1 1L9 9M9 1L1 9" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>
            </svg>
          </button>
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
  
  // 延迟显示，避免页面加载时闪烁
  setTimeout(() => {
    visible.value = true
  }, 1000)
})

function close() {
  visible.value = false
  localStorage.setItem('site-notice-closed-time', Date.now().toString())
}

defineExpose({ currentSite, sites })
</script>

<style scoped>
.notification-container {
  position: fixed;
  top: 12px;
  right: 24px;
  z-index: 1000;
  pointer-events: none;
}

.notification {
  position: relative;
  width: fit-content;
  min-width: 300px;
  max-width: 380px;
  padding: 14px 16px;
  background: var(--vp-c-bg);
  border-radius: 8px;
  box-shadow: 0 6px 24px rgba(0, 0, 0, 0.12);
  border: 1px solid var(--vp-c-divider);
  pointer-events: auto;
}

.notification-content {
  display: flex;
  gap: 12px;
}

.notification-icon {
  flex-shrink: 0;
  width: 24px;
  height: 24px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--vp-c-brand-1);
  background: var(--vp-c-brand-soft);
  border-radius: 50%;
  margin-top: 2px;
}

.notification-body {
  flex: 1;
  min-width: 0;
}

.notification-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--vp-c-text-1);
  line-height: 1.4;
  margin-bottom: 4px;
}

.notification-desc {
  font-size: 13px;
  color: var(--vp-c-text-2);
  line-height: 1.5;
  margin-bottom: 12px;
}

.site-links {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.site-link {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  padding: 3px 10px;
  font-size: 12px;
  font-weight: 500;
  color: var(--vp-c-text-1);
  background: var(--vp-c-bg-soft);
  border: 1px solid var(--vp-c-divider);
  border-radius: 4px;
  text-decoration: none;
  transition: all 0.2s ease;
  cursor: pointer;
}

.site-link:hover {
  color: var(--vp-c-brand-1);
  border-color: var(--vp-c-brand-1);
  background: var(--vp-c-brand-soft);
}

.site-link.active {
  color: var(--vp-c-brand-1);
  border-color: var(--vp-c-brand-1);
  background: var(--vp-c-brand-soft);
  font-weight: 600;
}

.site-check {
  font-size: 11px;
  margin-left: 2px;
}

.notification-close {
  position: absolute;
  top: 12px;
  right: 12px;
  width: 20px;
  height: 20px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: none;
  border: none;
  color: var(--vp-c-text-3);
  cursor: pointer;
  border-radius: 4px;
  transition: all 0.2s ease;
  padding: 0;
}

.notification-close:hover {
  color: var(--vp-c-text-1);
  background: var(--vp-c-bg-soft);
}

/* 动画 */
.notification-enter-active {
  animation: notificationIn 0.3s cubic-bezier(0.23, 1, 0.32, 1);
}

.notification-leave-active {
  animation: notificationOut 0.2s cubic-bezier(0.23, 1, 0.32, 1);
}

@keyframes notificationIn {
  0% {
    transform: translateX(100%);
    opacity: 0;
  }
  100% {
    transform: translateX(0);
    opacity: 1;
  }
}

@keyframes notificationOut {
  0% {
    transform: translateX(0);
    opacity: 1;
  }
  100% {
    transform: translateX(100%);
    opacity: 0;
  }
}

@media (max-width: 768px) {
  .notification-container {
    top: auto;
    bottom: 24px;
    right: 16px;
    left: 16px;
  }
  
  .notification {
    width: auto;
  }
}
</style>
