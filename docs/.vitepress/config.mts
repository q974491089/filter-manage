import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'Filter Manage - ICC配置文件管理 & NVIDIA颜色设置工具',
  description: 'Windows 桌面应用，一键切换 ICC 颜色配置文件，精细调节 NVIDIA 显卡亮度、对比度、伽马、数字振动(Digital Vibrance)，支持配置预设保存与游戏模式快速切换。免费开源的显卡颜色管理器。',
  base: '/',
  publicDir: 'public',
  lang: 'zh-CN',
  sitemap: {
    hostname: 'https://filter-manage.6ya.site'
  },
  head: [
    ['link', { rel: 'icon', href: '/favicon.png' }],
    ['meta', { name: 'keywords', content: 'ICC配置文件管理,ICC profile manager,ICC配置切换,NVIDIA颜色设置,NVIDIA color settings,数字振动,Digital Vibrance,显卡颜色调节,GPU color adjustment,亮度对比度伽马调节,brightness contrast gamma,Windows颜色管理,color management Windows,游戏画面调色,game color settings,显示器颜色校准,monitor color calibration,NVIDIA控制面板替代,色彩配置文件,color profile switcher,免费开源颜色工具' }],
    ['meta', { name: 'author', content: 'Filter Manage' }],
    ['meta', { property: 'og:title', content: 'Filter Manage - ICC配置文件管理 & NVIDIA显卡颜色设置工具 | 免费开源' }],
    ['meta', { property: 'og:description', content: 'Windows 桌面工具：一键切换 ICC 配置文件，精细调节 NVIDIA 亮度/对比度/伽马/数字振动，支持游戏模式预设。Free & open-source GPU color manager.' }],
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
      { text: '交流反馈', link: '/community' },
      {
        text: '镜像站点',
        items: [
          { text: '主站', link: 'https://filter-manage.6ya.site/' },
          { text: '镜像站点 1', link: 'https://filter-manage.vercel.app/' },
          { text: '镜像站点 2', link: 'https://filter-manage.xy18600.ggff.net/' },
          { text: '镜像站点 3', link: 'https://filter-manage.xyls.us.kg/' },
        ]
      },
    ],
    sidebar: [
      {
        text: '指南',
        items: [
          { text: '为什么选择 Filter Manage', link: '/why' },
          { text: '简介', link: '/guide/' },
          { text: '安装', link: '/guide/install' },
          { text: '使用方法', link: '/guide/usage' },
        ]
      },
      {
        text: '其他',
        items: [
          { text: '更新日志', link: '/changelog' },
          { text: '交流反馈', link: '/community' },
        ]
      }
    ],
    socialLinks: [
      { icon: 'github', link: 'https://github.com/q974491089/filter-manage' }
    ]
  }
})
