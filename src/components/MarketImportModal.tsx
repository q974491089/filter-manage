import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "../lib/ipc";
import {
  fetchMarketProfiles,
  marketAssetUrl,
  type MarketProfile,
} from "../lib/market";
import {
  getContributorId,
  getMySubmissions,
  type MySubmission,
} from "../lib/submissions";
import { debugNote } from "../lib/debug";
import { Icon } from "./Icon";
import SubmitIccModal from "./SubmitIccModal";
import { useModalAccessibility } from "../hooks/useModalAccessibility";

type ToastFn = (type: "success" | "error", text: string) => void;

interface MarketImportModalProps {
  open: boolean;
  onClose: () => void;
  onImported: () => void | Promise<void>;
  showToast: ToastFn;
  iccEffectPreview: string | null;
  /** 后台功能开关：市场投稿。false 时隐藏「我要提供」与「我的提交」。 */
  uploadEnabled?: boolean;
}

const PAGE_SIZE = 30;

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(2)} MB`;
}

function MarketImportModal({
  open,
  onClose,
  onImported,
  showToast,
  iccEffectPreview,
  uploadEnabled = true,
}: MarketImportModalProps) {
  const [items, setItems] = useState<MarketProfile[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [input, setInput] = useState("");
  const [query, setQuery] = useState("");
  const [importingId, setImportingId] = useState<string | null>(null);
  const [imported, setImported] = useState<Set<string>>(() => new Set());
  const [submitOpen, setSubmitOpen] = useState(false);
  const [tab, setTab] = useState<"market" | "mine">("market");
  const [mine, setMine] = useState<MySubmission[]>([]);
  const [mineLoading, setMineLoading] = useState(false);
  const [failedImageUrls, setFailedImageUrls] = useState<Set<string>>(() => new Set());
  const requestId = useRef(0);
  const modalRef = useModalAccessibility(open, onClose, !submitOpen);

  const load = useCallback(
    async (offset: number, append: boolean) => {
      const id = ++requestId.current;
      setLoading(true);
      setError(null);
      try {
        const data = await fetchMarketProfiles({
          q: query.trim() || undefined,
          limit: PAGE_SIZE,
          offset,
        });
        // 快速连续搜索时丢弃过期响应
        if (id !== requestId.current) return;
        setItems((prev) => (append ? [...prev, ...data.items] : data.items));
        setTotal(data.total);
      } catch (err) {
        if (id !== requestId.current) return;
        setError(err instanceof Error ? err.message : "市场加载失败");
      } finally {
        if (id === requestId.current) setLoading(false);
      }
    },
    [query]
  );

  // 打开或搜索词变化时从头加载
  useEffect(() => {
    if (!open) return;
    void load(0, false);
  }, [open, load]);

  // 每次重新打开都从干净的搜索状态开始；投稿被关闭时不允许停在「我的提交」
  useEffect(() => {
    if (!open) return;
    setInput("");
    setQuery("");
    setError(null);
    setTab("market");
  }, [open]);

  // 运行中开关被关掉（例如启动配置晚到）也要退回「市场」页签
  useEffect(() => {
    if (!uploadEnabled && tab === "mine") setTab("market");
  }, [uploadEnabled, tab]);

  const loadMine = useCallback(async () => {
    setMineLoading(true);
    try {
      const contributorId = await getContributorId();
      setMine(await getMySubmissions(contributorId));
    } catch (err) {
      showToast("error", err instanceof Error ? err.message : "加载提交记录失败");
    } finally {
      setMineLoading(false);
    }
  }, [showToast]);

  useEffect(() => {
    if (open && tab === "mine") void loadMine();
  }, [open, tab, loadMine]);

  const handleImport = async (profile: MarketProfile) => {
    if (importingId) return;
    setImportingId(profile.id);
    debugNote(`从市场导入 ICC「${profile.title}」`);
    try {
      const fileName = await invoke<string>("import_market_icc", {
        id: profile.id,
        title: profile.title,
      });
      setImported((prev) => new Set(prev).add(profile.id));
      showToast("success", `已导入「${fileName}」`);
      await onImported();
    } catch (err) {
      showToast("error", `导入失败: ${String(err)}`);
    } finally {
      setImportingId(null);
    }
  };

  if (!open) return null;

  const canLoadMore = items.length < total;

  return (
    <div
      data-components="MarketImportModal"
      data-name="market-overlay"
      className="fixed inset-0 z-[300] flex items-center justify-center bg-black/60 p-3 backdrop-blur-[2px] sm:p-5"
      onClick={onClose}
    >
      <div
        ref={modalRef}
        data-name="market-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="icc-market-title"
        aria-hidden={submitOpen}
        tabIndex={-1}
        onClick={(e) => e.stopPropagation()}
        className="bg-surface-container rounded-xl shadow-2xl border border-outline-variant/25 w-[960px] max-w-[94vw] h-[min(760px,90vh)] flex flex-col overflow-hidden"
      >
        {/* Header */}
        <div className="flex flex-wrap items-start justify-between gap-md px-lg pt-lg pb-md shrink-0 border-b border-outline-variant/15">
          <div className="min-w-0">
            <h2 id="icc-market-title" className="font-headline-sm text-headline-sm text-on-surface">ICC 市场</h2>
            <p className="mt-xs text-body-sm text-on-surface-variant">
              导入配置，或把你的校准文件提供给其他人
            </p>
          </div>
          <div className="flex flex-wrap items-center gap-sm">
            {uploadEnabled ? (
              <div role="tablist" aria-label="ICC 市场内容" className="flex items-center gap-1 rounded-lg bg-surface-container-high p-1">
                <button
                  id="icc-market-tab"
                  role="tab"
                  aria-selected={tab === "market"}
                  aria-controls="icc-market-panel"
                  tabIndex={tab === "market" ? 0 : -1}
                  onKeyDown={(event) => {
                    if (event.key === "ArrowRight" || event.key === "ArrowDown") {
                      event.preventDefault();
                      setTab("mine");
                    }
                  }}
                  onClick={() => setTab("market")}
                  className={`px-md py-sm rounded-md font-label-md text-label-md transition-colors duration-200 focus-visible:outline focus-visible:outline-2 focus-visible:outline-primary ${
                    tab === "market"
                      ? "bg-primary/12 text-primary"
                      : "text-on-surface-variant hover:bg-surface-variant/50"
                  }`}
                >
                  市场
                </button>
                <button
                  id="icc-submissions-tab"
                  role="tab"
                  aria-selected={tab === "mine"}
                  aria-controls="icc-submissions-panel"
                  tabIndex={tab === "mine" ? 0 : -1}
                  onKeyDown={(event) => {
                    if (event.key === "ArrowLeft" || event.key === "ArrowUp") {
                      event.preventDefault();
                      setTab("market");
                    }
                  }}
                  onClick={() => setTab("mine")}
                  className={`px-md py-sm rounded-md font-label-md text-label-md transition-colors duration-200 focus-visible:outline focus-visible:outline-2 focus-visible:outline-primary ${
                    tab === "mine"
                      ? "bg-primary/12 text-primary"
                      : "text-on-surface-variant hover:bg-surface-variant/50"
                  }`}
                >
                  我的提交
                </button>
              </div>
            ) : null}
            {uploadEnabled ? (
              <button
                data-name="submit-icc-button"
                onClick={() => setSubmitOpen(true)}
                className="flex items-center gap-xs px-md py-sm rounded-md bg-primary text-on-primary font-label-md text-label-md hover:brightness-110 transition-all duration-200 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary"
              >
                <Icon name="add" className="text-[16px]" />
                我要提供
              </button>
            ) : null}
            <button
              data-name="close-button"
              onClick={onClose}
              aria-label="关闭"
              className="p-xs rounded-md text-on-surface-variant hover:bg-surface-variant/60 hover:text-on-surface transition-colors"
            >
              <Icon name="close" className="text-[20px]" />
            </button>
          </div>
        </div>

        {tab === "market" ? (
          <>
        {/* Search */}
        <div id="icc-market-panel" role="tabpanel" aria-labelledby="icc-market-tab" aria-busy={loading} className="px-lg pt-md pb-sm shrink-0">
          <div className="flex items-center gap-sm px-md h-11 rounded-lg bg-surface-container-high border border-outline-variant/25 focus-within:border-primary/60 transition-colors duration-200">
            <Icon name="search" className="text-[18px] text-on-surface-variant" />
            <input
              data-name="market-search"
              aria-label="搜索 ICC 配置"
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") setQuery(input);
              }}
              placeholder="搜索名称、显示器型号或说明"
              className="flex-1 bg-transparent outline-none font-body-md text-body-md text-on-surface placeholder:text-on-surface-variant/50"
            />
            {input ? (
              <button
                onClick={() => {
                  setInput("");
                  setQuery("");
                }}
                aria-label="清空搜索"
                className="text-on-surface-variant hover:text-on-surface transition-colors"
              >
                <Icon name="close" className="text-[16px]" />
              </button>
            ) : null}
            <button
              data-name="search-button"
              onClick={() => setQuery(input)}
              className="shrink-0 px-sm py-xs rounded-md bg-primary/12 text-primary font-label-md text-label-md hover:bg-primary/20 transition-colors"
            >
              搜索
            </button>
          </div>
        </div>

        {/* List */}
        <div className="flex-1 min-h-0 overflow-y-auto px-lg pb-md">
          {loading && items.length === 0 ? (
            <div role="status" aria-live="polite" className="flex items-center justify-center py-16 text-on-surface-variant font-body-md text-body-md">
              <svg className="animate-spin w-5 h-5 mr-2" fill="none" viewBox="0 0 24 24">
                <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
              </svg>
              正在加载市场配置...
            </div>
          ) : error ? (
            <div className="text-center py-14">
              <p className="font-body-md text-body-md text-error mb-3">{error}</p>
              <button
                onClick={() => void load(0, false)}
                className="font-label-md text-label-md text-primary hover:text-primary-container transition-colors"
              >
                重新加载
              </button>
            </div>
          ) : items.length === 0 ? (
            <div className="text-center py-14">
              <Icon name="info" className="text-[40px] text-on-surface-variant/40" />
              <p className="mt-sm font-body-md text-body-md text-on-surface-variant">
                {query ? `没有找到与「${query}」匹配的配置` : "市场里还没有已发布的配置"}
              </p>
              <p className="mt-xs font-label-sm text-label-sm text-on-surface-variant/60">
                可以在管理后台上传并发布 ICC 配置
              </p>
            </div>
          ) : (
            <div className="grid grid-cols-1 gap-md sm:grid-cols-2">
              {items.map((profile) => {
                const isImported = imported.has(profile.id);
                const isImporting = importingId === profile.id;
                const imageKey = profile.thumbKey || profile.coverKey;
                const imageUrl = imageKey ? marketAssetUrl(imageKey) : "";
                return (
                  <div
                    key={profile.id}
                    data-name="market-item"
                    className="rounded-lg border border-outline-variant/20 bg-surface-container-high/40 overflow-hidden hover:border-primary/40 hover:bg-surface-container-high/70 transition-colors duration-200"
                  >
                    <div className="h-36 bg-surface-container-high sm:h-32">
                      {imageUrl && !failedImageUrls.has(imageUrl) ? (
                        <img
                          src={imageUrl}
                          alt={profile.title}
                          loading="lazy"
                          className="w-full h-full object-cover"
                          onError={() =>
                            setFailedImageUrls((current) => new Set(current).add(imageUrl))
                          }
                        />
                      ) : (
                        <div className="w-full h-full grid place-items-center text-on-surface-variant/40">
                          <Icon name="palette" className="text-[32px]" />
                        </div>
                      )}
                    </div>
                    <div className="p-md">
                      <div className="font-body-md font-semibold text-on-surface truncate" title={profile.title}>
                        {profile.title}
                      </div>
                      <div className="mt-xs text-body-sm text-on-surface-variant">
                        {profile.panel || "未标注型号"} · {formatBytes(profile.sizeBytes)} ·{" "}
                        {profile.downloadCount} 次下载
                      </div>
                      {profile.tags.length > 0 ? (
                        <div className="mt-xs flex flex-wrap gap-xs">
                          {profile.tags.slice(0, 3).map((tag) => (
                            <span
                              key={tag}
                              className="px-sm py-1 rounded-full bg-primary/10 text-primary text-xs"
                            >
                              {tag}
                            </span>
                          ))}
                        </div>
                      ) : null}
                      <div className="mt-sm flex items-center justify-between gap-sm">
                        <span className="text-xs text-on-surface-variant truncate">
                          {profile.contributorName ? `由 ${profile.contributorName} 提供` : ""}
                        </span>
                        <button
                          data-name="market-import-button"
                          onClick={() => void handleImport(profile)}
                          disabled={isImported || isImporting || !!importingId}
                          className={`shrink-0 min-h-9 flex items-center gap-xs px-md py-sm rounded-md font-label-md text-label-md transition-all duration-200 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary disabled:cursor-default ${
                            isImported
                              ? "bg-surface-variant/60 text-on-surface-variant"
                              : "bg-primary/12 text-primary hover:bg-primary/20 disabled:opacity-50"
                          }`}
                        >
                          <Icon
                            name={isImported ? "check_circle" : "download"}
                            className="text-[14px]"
                          />
                          {isImported ? "已导入" : isImporting ? "导入中..." : "导入"}
                        </button>
                      </div>
                    </div>
                  </div>
                );
              })}

              {canLoadMore ? (
                <button
                  data-name="load-more-button"
                  onClick={() => void load(items.length, true)}
                  disabled={loading}
                  className="col-span-2 w-full py-sm rounded-lg border border-outline-variant/25 text-on-surface-variant font-label-md text-label-md hover:bg-surface-variant/40 transition-colors disabled:opacity-50"
                >
                  {loading ? "加载中..." : `加载更多（已显示 ${items.length}/${total}）`}
                </button>
              ) : null}
            </div>
          )}
        </div>
          </>
        ) : (
          <div id="icc-submissions-panel" role="tabpanel" aria-labelledby="icc-submissions-tab" className="flex-1 min-h-0 overflow-y-auto px-lg py-md space-y-sm">
            <div className="flex justify-end">
              <button
                onClick={() => void loadMine()}
                disabled={mineLoading}
                className="px-sm py-xs rounded-md text-on-surface-variant hover:bg-surface-variant/50 font-label-md text-label-md disabled:opacity-50"
              >
                刷新
              </button>
            </div>
            {mineLoading ? (
              <p role="status" aria-live="polite" className="py-10 text-center text-on-surface-variant">加载中...</p>
            ) : mine.length === 0 ? (
              <p className="py-10 text-center text-on-surface-variant">还没有提交记录</p>
            ) : (
              mine.map((item) => (
                <div
                  key={item.id}
                  className="p-md rounded-lg border border-outline-variant/20 bg-surface-container-high/40"
                >
                  <div className="flex items-center justify-between gap-sm">
                    <span className="font-label-md text-label-md text-on-surface truncate">
                      {item.title}
                    </span>
                    <span
                      className={`shrink-0 text-[11px] font-label-md ${
                        item.status === "APPROVED"
                          ? "text-primary"
                          : item.status === "REJECTED"
                            ? "text-error"
                            : "text-on-surface-variant"
                      }`}
                    >
                      {item.status === "PENDING"
                        ? "审核中"
                        : item.status === "APPROVED"
                          ? "已通过"
                          : "已拒绝"}
                    </span>
                  </div>
                  {item.status === "REJECTED" && item.rejectReason ? (
                    <p className="mt-xs text-[11px] text-error">
                      拒绝理由：{item.rejectReason}
                    </p>
                  ) : null}
                </div>
              ))
            )}
          </div>
        )}

        {/* Footer */}
        <div className="shrink-0 px-lg py-md border-t border-outline-variant/15 flex flex-wrap items-center justify-between gap-xs text-xs text-on-surface-variant">
          <span>{total > 0 ? `共 ${total} 份已发布配置` : "ICC 市场"}</span>
          <span>导入后可点击左侧列表应用</span>
        </div>
      </div>

      <SubmitIccModal
        open={submitOpen}
        onClose={() => setSubmitOpen(false)}
        onSubmitted={() => {
          setSubmitOpen(false);
          setTab("mine");
          void loadMine();
        }}
        showToast={showToast}
        iccEffectPreview={iccEffectPreview}
      />
    </div>
  );
}

export default MarketImportModal;
