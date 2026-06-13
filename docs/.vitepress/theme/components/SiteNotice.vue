<template>
  <Teleport to="body">
    <Transition name="slide">
      <div class="site-notice" v-if="visible && isClient">
        <div class="notice-header">
          <span class="notice-icon">🌐</span>
          <span class="notice-title">镜像站点</span>
          <button class="close-btn" @click="close" aria-label="关闭">×</button>
        </div>
        <div class="notice-body">
          <p>如访问缓慢可尝试切换：</p>
          <div class="site-links">
            <a 
              v-for="site in sites" 
              :key="site.key"
              :href="site.url" 
              :class="['site-link', { active: currentSite === site.key }]"
            >
              {{ site.name }}
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

// 暴露当前站点信息供外部使用
defineExpose({ currentSite, sites })
</script>

<style scoped>
.site-notice {
  position: fixed;
  top: 80px;
  right: 20px;
  width: 280px;
  background: white;
  border-radius: 12px;
  box-shadow: 0 8px 30px rgba(0, 0, 0, 0.12), 0 2px 8px rgba(0, 0, 0, 0.08);
  z-index: 1000;
  overflow: hidden;
  border: 1px solid rgba(0, 0, 0, 0.06);
}

.notice-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 16px;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  color: white;
}

.notice-icon {
  font-size: 16px;
}

.notice-title {
  flex: 1;
  font-weight: 600;
  font-size: 14px;
}

.close-btn {
  background: none;
  border: none;
  color: rgba(255, 255, 255, 0.8);
  font-size: 18px;
  cursor: pointer;
  padding: 0;
  line-height: 1;
  transition: color 0.2s ease;
}

.close-btn:hover {
  color: white;
}

.notice-body {
  padding: 14px 16px;
}

.notice-body p {
  margin: 0 0 12px 0;
  font-size: 13px;
  color: #666;
}

.site-links {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.site-link {
  display: block;
  padding: 8px 12px;
  color: #333;
  text-decoration: none;
  font-size: 13px;
  font-weight: 500;
  border-radius: 6px;
  transition: all 0.2s ease;
  background: #f5f5f5;
}

.site-link:hover {
  background: #e8e8e8;
  color: #667eea;
}

.site-link.active {
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  color: white;
}

/* 动画 */
.slide-enter-active {
  animation: slideIn 0.3s ease;
}

.slide-leave-active {
  animation: slideOut 0.2s ease;
}

@keyframes slideIn {
  from {
    transform: translateX(100%);
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
    transform: translateX(100%);
    opacity: 0;
  }
}

/* 暗色模式 */
:root.dark .site-notice {
  background: #1e1e1e;
  border-color: rgba(255, 255, 255, 0.1);
}

:root.dark .notice-body p {
  color: #999;
}

:root.dark .site-link {
  background: #2a2a2a;
  color: #e0e0e0;
}

:root.dark .site-link:hover {
  background: #333;
  color: #8b9cf7;
}

:root.dark .site-link.active {
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  color: white;
}
</style>
