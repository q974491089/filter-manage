import { sha256 } from "@noble/hashes/sha2.js";
import { bytesToHex } from "@noble/hashes/utils.js";
import { invoke } from "./ipc";

const MARKET_BASE = "https://filter-manage-admin.xyls.us.kg";

export type SubmissionStatus = "PENDING" | "APPROVED" | "REJECTED";

export interface MySubmission {
  id: string;
  title: string;
  status: SubmissionStatus;
  rejectReason: string;
  createdAt: string;
}

export interface SubmissionTokens {
  iccToken: string;
  imageToken: string;
  expiresAt: string;
  gatewayUrl: string;
  imageGatewayUrl: string;
  maxIccBytes: number;
  maxImageBytes: number;
}

export interface CreateSubmissionPayload {
  contributorId: string;
  nickname: string;
  sha256: string;
  title: string;
  description: string;
  panel: string;
  tags: string[];
  coverKey: string;
  thumbKey: string;
  clientVersion: string;
}

export async function getContributorId(): Promise<string> {
  return invoke<string>("get_or_create_contributor_id");
}

export async function getSubmissionTokens(
  contributorId: string,
  nickname: string,
): Promise<SubmissionTokens> {
  const res = await fetch(`${MARKET_BASE}/api/market/submissions/token`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ contributorId, nickname }),
  });
  if (!res.ok) throw new Error(`获取投稿令牌失败（HTTP ${res.status}）`);
  return (await res.json()) as SubmissionTokens;
}

export async function uploadToGateway(
  url: string,
  token: string,
  sha256: string,
  buffer: ArrayBuffer,
  contentType: string,
): Promise<{ key: string; duplicate: boolean }> {
  const res = await fetch(url, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${token}`,
      "Content-Type": contentType,
      "X-Content-SHA256": sha256,
    },
    body: buffer,
  });
  const data = await res.json().catch(() => null);
  if (!res.ok) {
    throw new Error(
      data?.error ? `上传失败：${data.error}` : `上传失败（HTTP ${res.status}）`,
    );
  }
  return { key: String(data?.key ?? ""), duplicate: Boolean(data?.duplicate) };
}

export async function createSubmission(payload: CreateSubmissionPayload) {
  const res = await fetch(`${MARKET_BASE}/api/market/submissions`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(payload),
  });
  const data = await res.json().catch(() => null);
  if (!res.ok) {
    throw new Error(data?.message || `提交失败（HTTP ${res.status}）`);
  }
  return data.item;
}

export async function getMySubmissions(
  contributorId: string,
): Promise<MySubmission[]> {
  const res = await fetch(
    `${MARKET_BASE}/api/market/submissions?contributorId=${encodeURIComponent(contributorId)}`,
  );
  if (!res.ok) throw new Error(`加载提交记录失败（HTTP ${res.status}）`);
  const data = await res.json();
  return data.items as MySubmission[];
}

export async function sha256Hex(buffer: ArrayBuffer): Promise<string> {
  // 纯 JS 实现，避免依赖 WebView 的 secure context / crypto.subtle
  return bytesToHex(sha256(new Uint8Array(buffer)));
}
