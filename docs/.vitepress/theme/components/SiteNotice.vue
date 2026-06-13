<template>
  <div class="site-notice" v-if="visible">
    <div class="notice-content">
      <span class="notice-icon">🌐</span>
      <span class="notice-text">
        本站提供多个镜像站点，如访问缓慢可尝试切换：
        <a href="https://filter-manage.6ya.site/" :class="{ active: currentSite === '6ya.site' }">主站</a>
        <span class="divider">·</span>
        <a href="https://filter-manage.vercel.app/" :class="{ active: currentSite === 'vercel.app' }">Vercel</a>
        <span class="divider">·</span>
        <a href="https://filter-manage.xyls.us.kg/" :class="{ active: currentSite === 'xyls.us.kg' }">Cloudflare</a>
      </span>
      <button class="close-btn" @click="close" aria-label="关闭">×</button>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'

const visible = ref(true)
const currentSite = ref('')

onMounted(() => {
  const host = window.location.hostname
  if (host.includes('6ya.site')) currentSite.value = '6ya.site'
  else if (host.includes('vercel.app')) currentSite.value = 'vercel.app'
  else if (host.includes('xyls.us.kg')) currentSite.value = 'xyls.us.kg'
  else currentSite.value = host

  // 检查是否已关闭过
  const closed = localStorage.getItem('site-notice-closed')
  if (closed === 'true') visible.value = false
})

function close() {
  visible.value = false
  localStorage.setItem('site-notice-closed', 'true')
}
</script>

<style scoped>
.site-notice {
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  color: white;
  padding: 0;
  margin-bottom: 20px;
  border-radius: 8px;
  overflow: hidden;
}

.notice-content {
  max-width: 1200px;
  margin: 0 auto;
  padding: 10px 24px;
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 14px;
}

.notice-icon {
  font-size: 18px;
  flex-shrink: 0;
}

.notice-text {
  flex: 1;
  line-height: 1.5;
}

.notice-text a {
  color: rgba(255, 255, 255, 0.9);
  text-decoration: none;
  font-weight: 500;
  padding: 2px 8px;
  border-radius: 4px;
  transition: all 0.2s ease;
}

.notice-text a:hover {
  color: white;
  background: rgba(255, 255, 255, 0.2);
}

.notice-text a.active {
  color: white;
  background: rgba(255, 255, 255, 0.3);
  font-weight: 600;
}

.divider {
  opacity: 0.6;
  margin: 0 2px;
}

.close-btn {
  background: none;
  border: none;
  color: rgba(255, 255, 255, 0.8);
  font-size: 20px;
  cursor: pointer;
  padding: 0 4px;
  line-height: 1;
  transition: color 0.2s ease;
  flex-shrink: 0;
}

.close-btn:hover {
  color: white;
}

@media (max-width: 768px) {
  .notice-content {
    padding: 8px 16px;
    font-size: 13px;
    flex-wrap: wrap;
  }
  
  .notice-text {
    flex-basis: calc(100% - 40px);
  }
}
</style>
