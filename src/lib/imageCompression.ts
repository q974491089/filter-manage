import imageCompression from "browser-image-compression";

export interface PreparedImage {
  cover: File;
  thumb: File;
}

async function compress(
  file: File,
  maxEdge: number,
  quality: number,
): Promise<File> {
  return imageCompression(file, {
    maxWidthOrHeight: maxEdge,
    fileType: "image/webp",
    initialQuality: quality,
    // Tauri 离线环境不依赖 CDN 上的 worker 脚本；单张图在主线程压缩也足够快
    useWebWorker: false,
  });
}

/** 封面 1600px / 0.9，缩略图 480px / 0.85；返回两个 WebP 文件。 */
export async function prepareCover(file: File): Promise<PreparedImage> {
  if (!file.type.startsWith("image/")) {
    throw new Error("请选择图片文件");
  }
  const cover = await compress(file, 1600, 0.9);
  const thumb = await compress(file, 480, 0.85);
  return { cover, thumb };
}
