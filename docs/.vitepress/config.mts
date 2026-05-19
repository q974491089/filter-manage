import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'Filter Manage',
  description: 'Windows ICC 颜色配置文件管理 & NVIDIA 显卡颜色设置工具，支持一键切换 ICC 配置、亮度对比度伽马数字振动调节、配置预设保存',
  base: '/',
  publicDir: 'public',
  lang: 'zh-CN',
  sitemap: {
    hostname: 'https://filter-manage.6ya.site'
  },
  head: [
    ['link', { rel: 'icon', href: '/favicon.png' }],
    ['meta', { name: 'keywords', content: 'ICC配置文件管理,NVIDIA颜色设置,数字振动,Digital Vibrance,显卡颜色调节,ICC profile manager,NVIDIA color settings,Windows颜色管理,游戏画面调色,伽马调节' }],
    ['meta', { name: 'author', content: 'Filter Manage' }],
    ['meta', { property: 'og:title', content: 'Filter Manage - ICC & NVIDIA 颜色设置管理器' }],
    ['meta', { property: 'og:description', content: 'Windows 桌面工具：一键切换 ICC 配置文件，精细调节 NVIDIA 亮度/对比度/伽马/数字振动' }],
    ['meta', { property: 'og:url', content: 'https://filter-manage.6ya.site/' }],
    ['meta', { property: 'og:type', content: 'website' }],
    ['meta', { property: 'og:locale', content: 'zh_CN' }],
    ['meta', { name: 'robots', content: 'index, follow' }],
    ['link', { rel: 'canonical', href: 'https://filter-manage.6ya.site/' }],
  ],
  themeConfig: {
    logo: '/favicon.png',
    nav: [
      { text: '首页', link: '/' },
      { text: '使用指南', link: '/guide/' },
      { text: '更新日志', link: '/changelog' },
    ],
    sidebar: [
      {
        text: '指南',
        items: [
          { text: '简介', link: '/guide/' },
          { text: '安装', link: '/guide/install' },
          { text: '使用方法', link: '/guide/usage' },
        ]
      },
      {
        text: '其他',
        items: [
          { text: '更新日志', link: '/changelog' },
        ]
      }
    ],
    socialLinks: [
      { icon: 'github', link: 'https://github.com/q974491089/filter-manage' }
    ]
  }
})
