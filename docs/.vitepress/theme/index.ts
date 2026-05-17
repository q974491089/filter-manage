import DefaultTheme from 'vitepress/theme'
import './custom.css'
import ThemeCarousel from './components/ThemeCarousel.vue'

export default {
  extends: DefaultTheme,
  enhanceApp({ app }) {
    app.component('ThemeCarousel', ThemeCarousel)
  }
}
