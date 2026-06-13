<template>
  <Layout>
    <template #doc-top>
      <SiteNotice />
    </template>
  </Layout>
</template>

<script setup>
import DefaultTheme from 'vitepress/theme'
import SiteNotice from './components/SiteNotice.vue'

const { Layout } = DefaultTheme
</script>
