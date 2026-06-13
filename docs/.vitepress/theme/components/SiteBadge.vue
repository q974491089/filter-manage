<template>
  <span class="site-badge" v-if="isClient && currentSiteName">
    {{ currentSiteName }}
  </span>
</template>

<script setup>
import { ref, onMounted, computed } from 'vue'

const isClient = ref(false)
const currentSite = ref('')

const siteMap = {
  '6ya.site': '主站',
  'vercel.app': '镜像 1',
  'xy18600.ggff.net': '镜像 2',
  'xyls.us.kg': '镜像 3',
}

const currentSiteName = computed(() => {
  return siteMap[currentSite.value] || ''
})

onMounted(() => {
  isClient.value = true
  const host = window.location.hostname
  
  if (host.includes('6ya.site')) currentSite.value = '6ya.site'
  else if (host.includes('xy18600.ggff.net')) currentSite.value = 'xy18600.ggff.net'
  else if (host.includes('vercel.app')) currentSite.value = 'vercel.app'
  else if (host.includes('xyls.us.kg')) currentSite.value = 'xyls.us.kg'
})
</script>

<style scoped>
.site-badge {
  display: inline-block;
  padding: 2px 10px;
  margin-left: 12px;
  font-size: 12px;
  font-weight: 500;
  color: #667eea;
  background: rgba(102, 126, 234, 0.1);
  border: 1px solid rgba(102, 126, 234, 0.2);
  border-radius: 20px;
  vertical-align: middle;
  letter-spacing: 0.5px;
}

:root.dark .site-badge {
  color: #8b9cf7;
  background: rgba(139, 156, 247, 0.1);
  border-color: rgba(139, 156, 247, 0.2);
}
</style>
