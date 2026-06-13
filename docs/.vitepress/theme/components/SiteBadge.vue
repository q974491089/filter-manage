<template>
  <!-- 空组件，使用 CSS 伪元素实现 -->
</template>

<script setup>
import { ref, onMounted, watch } from 'vue'

const siteMap = {
  '6ya.site': '主站',
  'vercel.app': '镜像 1',
  'xy18600.ggff.net': '镜像 2',
  'xyls.us.kg': '镜像 3',
}

onMounted(() => {
  const host = window.location.hostname
  let siteName = ''
  
  if (host.includes('6ya.site')) siteName = siteMap['6ya.site']
  else if (host.includes('xy18600.ggff.net')) siteName = siteMap['xy18600.ggff.net']
  else if (host.includes('vercel.app')) siteName = siteMap['vercel.app']
  else if (host.includes('xyls.us.kg')) siteName = siteMap['xyls.us.kg']
  
  if (siteName) {
    document.documentElement.style.setProperty('--site-badge', `"${siteName}"`)
    document.documentElement.classList.add('has-site-badge')
  }
})
</script>
