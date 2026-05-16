import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'Filter Manage',
  description: 'ICC & NVIDIA Color Settings Manager',
  base: '/filter-manage/',
  themeConfig: {
    nav: [
      { text: '首页', link: '/' },
      { text: '使用指南', link: '/guide/' },
    ],
    sidebar: [
      {
        text: '指南',
        items: [
          { text: '简介', link: '/guide/' },
          { text: '安装', link: '/guide/install' },
          { text: '使用方法', link: '/guide/usage' },
        ]
      }
    ],
    socialLinks: [
      { icon: 'github', link: 'https://github.com/q974491089/filter-manage' }
    ]
  }
})
