<template>
  <Layout>
    <template #home-hero-before>
      <SiteNotice />
    </template>
    <template #nav-bar-title-after>
      <SiteBadge />
    </template>
  </Layout>
</template>

<script setup>
import DefaultTheme from 'vitepress/theme'
import SiteNotice from './components/SiteNotice.vue'
import SiteBadge from './components/SiteBadge.vue'

const { Layout } = DefaultTheme
</script>
