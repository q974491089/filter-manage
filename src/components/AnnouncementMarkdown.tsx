import type { Components } from "react-markdown";
import Markdown from "react-markdown";
import { openUrl } from "@tauri-apps/plugin-opener";

/**
 * 公告正文 Markdown：外链经 opener 用系统浏览器打开（Tauri WebView 内默认 a 标签不可靠）。
 */
const components: Components = {
  a: ({ href, children }) => (
    <a
      href={href || "#"}
      className="text-primary underline underline-offset-2 hover:opacity-90 cursor-pointer break-all"
      onClick={(e) => {
        e.preventDefault();
        e.stopPropagation();
        if (!href) return;
        // 仅允许 http(s)，避免 javascript: 等
        if (!/^https?:\/\//i.test(href)) return;
        void openUrl(href);
      }}
    >
      {children}
    </a>
  ),
};

export default function AnnouncementMarkdown({ children }: { children: string }) {
  return (
    <div
      data-component="AnnouncementMarkdown"
      className="prose prose-invert prose-sm max-w-none text-on-surface-variant prose-a:text-primary"
    >
      <Markdown components={components}>{children}</Markdown>
    </div>
  );
}
