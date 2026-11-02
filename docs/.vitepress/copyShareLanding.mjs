import { copyFileSync, mkdirSync } from 'node:fs'
import { join } from 'node:path'

/**
 * 把方案分享落地页的静态资产复制进构建产物：
 *
 * - share-shell.html  → dist/s/index.html（/s/<slug> 的静态壳，Edge Function
 *   会先取这份 HTML、注入 <title>/og:* 再返回；镜像站没有函数时直接当静态页兜底）
 * - public/share-landing.js → dist/share-landing.js（壳页面引用的渲染脚本）
 *
 * 挂在 buildEnd 是因为这两个文件不属于任何 VitePress 页面，走 markdown/
 * publicDir 流程都会被加上多余处理或丢失目录结构。
 */
export function copyShareLanding(siteConfig) {
  const vpDir = siteConfig.srcDir.replace(/[\\/]+$/, '')
  const outDir = siteConfig.outDir
  const shellSrc = join(vpDir, '.vitepress', 'share-shell.html')

  mkdirSync(join(outDir, 's'), { recursive: true })
  copyFileSync(
    shellSrc,
    join(outDir, 's', 'index.html'),
  )
  copyFileSync(
    join(vpDir, 'public', 'share-landing.js'),
    join(outDir, 'share-landing.js'),
  )
  copyFileSync(
    join(vpDir, 'public', 'share-landing.css'),
    join(outDir, 'share-landing.css'),
  )
}
