import DefaultTheme from 'vitepress/theme'
import './custom.css'
import ThemeCarousel from './components/ThemeCarousel.vue'
import SiteNotice from './components/SiteNotice.vue'
import Layout from './Layout.vue'

export default {
  Layout,
  enhanceApp({ app }) {
    app.component('ThemeCarousel', ThemeCarousel)
    app.component('SiteNotice', SiteNotice)
  }
}
