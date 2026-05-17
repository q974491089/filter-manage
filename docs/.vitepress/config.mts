import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'Filter Manage',
  description: 'ICC & NVIDIA Color Settings Manager',
  base: '/',
  publicDir: 'public',
  head: [
    ['link', { rel: 'icon', href: '/favicon.png' }],
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
