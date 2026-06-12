# Filter Manage 云端工坊功能设计方案

## 一、功能概述

云端工坊是一个配色预设分享平台，允许用户：
- 上传自己的配色预设到云端
- 浏览和下载其他用户分享的预设
- 点赞、收藏、评论优质预设
- 搜索和筛选预设（按游戏、用途、标签等）

## 二、核心功能模块

### 2.1 用户系统
- **匿名浏览**：无需登录即可浏览和下载预设
- **快速登录**：支持 GitHub OAuth / 邮箱验证码登录
- **用户信息**：昵称、头像、个人简介
- **用户主页**：展示用户上传的所有预设

### 2.2 预设管理
- **上传预设**：从本地预设一键上传到云端
- **预设信息**：
  - 基础信息：名称、描述、标签、适用场景（游戏/电影/护眼等）
  - 技术参数：brightness、contrast、gamma、digital_vibrance、icc_profile
  - 元数据：上传者、上传时间、下载次数、点赞数
  - 预览图：可选的效果对比图
- **版本管理**：同一预设可以更新版本

### 2.3 社区互动
- **点赞收藏**：支持点赞和收藏优质预设
- **评论反馈**：用户可以评论预设效果
- **排行榜**：热门预设、最新预设、周榜月榜
- **标签系统**：#CS2 #Valorant #护眼 #电影 #摄影 等

### 2.4 搜索与筛选
- **关键词搜索**：搜索预设名称、描述、标签
- **分类筛选**：按用途（游戏/电影/护眼等）筛选
- **游戏筛选**：热门游戏专属标签
- **排序方式**：最新、最热、最受欢迎

## 三、技术架构

### 3.1 前端（Tauri App）

#### 新增页面/组件
```
src/
├── pages/
│   └── Workshop.tsx          # 云端工坊主页面
├── components/
│   ├── workshop/
│   │   ├── PresetCard.tsx    # 预设卡片组件
│   │   ├── PresetDetail.tsx  # 预设详情弹窗
│   │   ├── UploadDialog.tsx  # 上传预设对话框
│   │   ├── SearchBar.tsx     # 搜索栏
│   │   ├── FilterPanel.tsx   # 筛选面板
│   │   └── UserProfile.tsx   # 用户资料页
│   └── auth/
│       └── LoginDialog.tsx   # 登录对话框
```

#### 状态管理
使用 Zustand 或 Context API 管理：
- 用户登录状态
- 云端预设列表
- 本地与云端同步状态

#### API 集成
```typescript
// src/api/workshop.ts
export interface CloudPreset {
  id: string;
  name: string;
  description: string;
  tags: string[];
  category: 'gaming' | 'movie' | 'reading' | 'photo' | 'design' | 'other';
  author: {
    id: string;
    name: string;
    avatar?: string;
  };
  config: ColorConfig;
  preview_image?: string;
  stats: {
    downloads: number;
    likes: number;
    views: number;
  };
  created_at: string;
  updated_at: string;
}

export const workshopApi = {
  // 获取预设列表
  getPresets: (params: { 
    page: number; 
    category?: string; 
    tags?: string[]; 
    sort?: 'latest' | 'popular' 
  }) => Promise<CloudPreset[]>,
  
  // 获取预设详情
  getPreset: (id: string) => Promise<CloudPreset>,
  
  // 上传预设
  uploadPreset: (data: { 
    config: ColorConfig; 
    description: string; 
    tags: string[]; 
    category: string; 
    preview?: File 
  }) => Promise<CloudPreset>,
  
  // 下载预设（保存到本地）
  downloadPreset: (id: string) => Promise<ColorConfig>,
  
  // 点赞/取消点赞
  likePreset: (id: string, like: boolean) => Promise<void>,
  
  // 收藏/取消收藏
  favoritePreset: (id: string, favorite: boolean) => Promise<void>,
  
  // 搜索预设
  searchPresets: (query: string) => Promise<CloudPreset[]>,
};
```

### 3.2 后端架构选择

#### 方案 A：无服务器架构（推荐）

**技术栈：**
- **存储**：Cloudflare R2 / AWS S3（预设数据 JSON + 预览图）
- **数据库**：Cloudflare D1 / PlanetScale（MySQL）
- **API**：Cloudflare Workers / Vercel Serverless Functions
- **认证**：Clerk / Auth0 / Supabase Auth
- **CDN**：Cloudflare CDN

**优势：**
- 零运维成本
- 自动扩展
- 按使用量付费（初期几乎免费）
- 全球 CDN 加速
- 开发快速

**成本估算（月）：**
- Cloudflare Workers：免费额度内（10万请求/天）
- Cloudflare D1：免费额度内（500万次读取/月）
- Cloudflare R2：免费额度内（10GB 存储）
- **总计：$0 - $5/月**

#### 方案 B：传统服务器架构

**技术栈：**
- **后端框架**：Rust (Axum) / Node.js (Express)
- **数据库**：PostgreSQL / MySQL
- **对象存储**：MinIO（自建）/ 云厂商 OSS
- **服务器**：轻量云服务器（阿里云/腾讯云）

**优势：**
- 完全掌控
- 可定制性强
- 无供应商锁定

**成本估算（月）：**
- 云服务器（2核4G）：¥50-100
- 数据库：¥30-50
- 对象存储：¥10-30
- **总计：¥90-180/月**

### 3.3 数据库设计

```sql
-- 用户表
CREATE TABLE users (
  id VARCHAR(36) PRIMARY KEY,
  username VARCHAR(50) UNIQUE NOT NULL,
  email VARCHAR(255) UNIQUE,
  avatar_url TEXT,
  bio TEXT,
  github_id VARCHAR(50) UNIQUE,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
);

-- 预设表
CREATE TABLE presets (
  id VARCHAR(36) PRIMARY KEY,
  name VARCHAR(100) NOT NULL,
  description TEXT,
  category ENUM('gaming', 'movie', 'reading', 'photo', 'design', 'other') NOT NULL,
  author_id VARCHAR(36) NOT NULL,
  
  -- 颜色配置（JSON）
  config JSON NOT NULL,
  
  -- 预览图
  preview_image_url TEXT,
  
  -- 统计数据
  downloads INT DEFAULT 0,
  likes INT DEFAULT 0,
  views INT DEFAULT 0,
  
  -- 版本管理
  version VARCHAR(20) DEFAULT '1.0.0',
  parent_id VARCHAR(36),
  
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  
  FOREIGN KEY (author_id) REFERENCES users(id) ON DELETE CASCADE,
  FOREIGN KEY (parent_id) REFERENCES presets(id) ON DELETE SET NULL,
  INDEX idx_category (category),
  INDEX idx_author (author_id),
  INDEX idx_created (created_at DESC),
  INDEX idx_downloads (downloads DESC),
  INDEX idx_likes (likes DESC)
);

-- 标签表
CREATE TABLE tags (
  id INT AUTO_INCREMENT PRIMARY KEY,
  name VARCHAR(50) UNIQUE NOT NULL,
  usage_count INT DEFAULT 0,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- 预设-标签关联表
CREATE TABLE preset_tags (
  preset_id VARCHAR(36) NOT NULL,
  tag_id INT NOT NULL,
  PRIMARY KEY (preset_id, tag_id),
  FOREIGN KEY (preset_id) REFERENCES presets(id) ON DELETE CASCADE,
  FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
);

-- 点赞表
CREATE TABLE preset_likes (
  user_id VARCHAR(36) NOT NULL,
  preset_id VARCHAR(36) NOT NULL,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (user_id, preset_id),
  FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
  FOREIGN KEY (preset_id) REFERENCES presets(id) ON DELETE CASCADE
);

-- 收藏表
CREATE TABLE preset_favorites (
  user_id VARCHAR(36) NOT NULL,
  preset_id VARCHAR(36) NOT NULL,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (user_id, preset_id),
  FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
  FOREIGN KEY (preset_id) REFERENCES presets(id) ON DELETE CASCADE
);

-- 评论表
CREATE TABLE comments (
  id VARCHAR(36) PRIMARY KEY,
  preset_id VARCHAR(36) NOT NULL,
  user_id VARCHAR(36) NOT NULL,
  content TEXT NOT NULL,
  parent_id VARCHAR(36),
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  FOREIGN KEY (preset_id) REFERENCES presets(id) ON DELETE CASCADE,
  FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
  FOREIGN KEY (parent_id) REFERENCES comments(id) ON DELETE CASCADE
);
```

### 3.4 API 设计

#### RESTful API 端点

```
基础路径：https://api.filtermanage.com/v1

认证方式：JWT Bearer Token

# 预设相关
GET    /presets                  # 获取预设列表
  Query: page, limit, category, tags[], sort, search
  
GET    /presets/:id              # 获取单个预设详情
POST   /presets                  # 上传新预设（需认证）
PUT    /presets/:id              # 更新预设（需认证，仅作者）
DELETE /presets/:id              # 删除预设（需认证，仅作者）
POST   /presets/:id/download     # 记录下载（无需认证）

# 互动相关
POST   /presets/:id/like         # 点赞（需认证）
DELETE /presets/:id/like         # 取消点赞（需认证）
POST   /presets/:id/favorite     # 收藏（需认证）
DELETE /presets/:id/favorite     # 取消收藏（需认证）
GET    /presets/:id/comments     # 获取评论
POST   /presets/:id/comments     # 发表评论（需认证）

# 用户相关
GET    /users/:id                # 获取用户信息
GET    /users/:id/presets        # 获取用户的预设
GET    /users/me                 # 获取当前用户信息（需认证）
PUT    /users/me                 # 更新用户信息（需认证）
GET    /users/me/favorites       # 获取收藏列表（需认证）

# 标签相关
GET    /tags                     # 获取热门标签
GET    /tags/:name/presets       # 获取某标签下的预设

# 统计相关
GET    /stats/trending           # 获取趋势预设
GET    /stats/popular-games      # 获取热门游戏标签
```

## 四、实施步骤

### Phase 1：基础架构搭建（第1-2周）
1. 确定技术栈和云服务商
2. 搭建后端 API 框架
3. 设计并创建数据库表结构
4. 配置对象存储（R2/S3）
5. 实现基础认证系统

### Phase 2：核心功能开发（第3-4周）
1. 实现预设上传/下载 API
2. 实现预设列表/详情 API
3. 前端：工坊主页面 UI
4. 前端：预设卡片和列表组件
5. 前端：预设详情弹窗

### Phase 3：社区功能（第5周）
1. 实现点赞/收藏功能
2. 实现评论系统
3. 实现用户主页
4. 前端对应组件开发

### Phase 4：搜索与优化（第6周）
1. 实现搜索功能
2. 实现标签系统
3. 添加排序和筛选
4. 性能优化和 CDN 配置

### Phase 5：测试与发布（第7周）
1. 集成测试
2. UI/UX 优化
3. 编写使用文档
4. Beta 测试
5. 正式发布

## 五、用户体验设计

### 5.1 工坊入口
在主界面添加"云端工坊"标签页：
```
┌─────────────────────────────────────┐
│  [色彩预设]  [云端工坊]  [设置]      │
└─────────────────────────────────────┘
```

### 5.2 工坊主界面布局
```
┌──────────────────────────────────────────────────┐
│  🔍 搜索框                      [登录] [上传预设] │
├──────────────────────────────────────────────────┤
│  分类：[全部] [游戏] [电影] [护眼] [摄影] [设计] │
│  标签：#CS2 #Valorant #LOL #电影 #护眼           │
│  排序：[最新] [最热] [下载量]                     │
├──────────────────────────────────────────────────┤
│  ┌────────┐  ┌────────┐  ┌────────┐             │
│  │ 预设卡 │  │ 预设卡 │  │ 预设卡 │             │
│  │  片1   │  │  片2   │  │  片3   │             │
│  └────────┘  └────────┘  └────────┘             │
│  ┌────────┐  ┌────────┐  ┌────────┐             │
│  │ 预设卡 │  │ 预设卡 │  │ 预设卡 │             │
│  │  片4   │  │  片5   │  │  片6   │             │
│  └────────┘  └────────┘  └────────┘             │
└──────────────────────────────────────────────────┘
```

### 5.3 预设卡片设计
```
┌─────────────────────────────────┐
│  [预览图/占位图]                 │
├─────────────────────────────────┤
│  预设名称                        │
│  by 作者名 · 2天前               │
├─────────────────────────────────┤
│  #CS2 #竞技 #锐化               │
├─────────────────────────────────┤
│  ❤️ 256  💾 1.2k  👁 3.5k       │
│           [下载] [详情]          │
└─────────────────────────────────┘
```

### 5.4 预设详情弹窗
```
┌─────────────────────────────────────────────┐
│  [X]                                         │
│  预设名称                        by 作者名   │
│  ─────────────────────────────────────────  │
│  [预览图对比滑块]                            │
│  ─────────────────────────────────────────  │
│  描述：这是一个适合 CS2 的锐化预设...        │
│  ─────────────────────────────────────────  │
│  参数详情：                                  │
│    亮度：+20  对比度：+15                    │
│    伽马：1.2  数字振动：65                   │
│    ICC：sRGB.icc                             │
│  ─────────────────────────────────────────  │
│  标签：#CS2 #竞技 #锐化                      │
│  ─────────────────────────────────────────  │
│  ❤️ 点赞 256  ⭐ 收藏  💬 评论 (12)          │
│  ─────────────────────────────────────────  │
│  [立即下载并应用]  [仅下载到本地]            │
└─────────────────────────────────────────────┘
```

## 六、安全与隐私

### 6.1 安全措施
- **API 限流**：防止滥用和 DDoS
- **内容审核**：预设名称和描述敏感词过滤
- **文件校验**：上传文件格式和大小限制
- **SQL 注入防护**：使用参数化查询
- **XSS 防护**：前端输入验证和转义

### 6.2 隐私保护
- **匿名下载**：无需登录即可下载
- **最小权限**：仅必要时要求登录
- **数据透明**：用户可删除自己的预设和数据
- **不追踪**：不收集设备指纹和隐私数据

## 七、运营与增长

### 7.1 内容冷启动
1. 官方创建 20-30 个高质量预设
2. 邀请社区意见领袖上传预设
3. 举办"最佳预设"评选活动

### 7.2 用户激励
- 上传预设获得徽章
- 热门预设作者展示
- 月度最佳创作者奖励

### 7.3 社区建设
- 建立 Discord/QQ 群
- 定期预设推荐
- 用户反馈渠道

## 八、后续扩展功能

### 短期（3个月内）
- [ ] 预设版本历史
- [ ] 批量导入导出
- [ ] 预设对比功能
- [ ] 移动端 Web 版

### 中期（6个月内）
- [ ] AI 推荐系统
- [ ] 预设组合（多场景切换）
- [ ] 云端同步配置
- [ ] 社区排行榜

### 长期（1年内）
- [ ] 插件市场（自定义滤镜）
- [ ] 付费专业预设
- [ ] API 开放平台
- [ ] 多平台支持（macOS/Linux）

## 九、技术细节参考

### 9.1 Cloudflare Workers 示例代码

```typescript
// worker/index.ts
import { Router } from 'itty-router';

const router = Router();

// 获取预设列表
router.get('/api/v1/presets', async (request, env) => {
  const { searchParams } = new URL(request.url);
  const page = parseInt(searchParams.get('page') || '1');
  const limit = parseInt(searchParams.get('limit') || '20');
  const category = searchParams.get('category');
  
  // 从 D1 查询
  const query = `
    SELECT 
      p.*, 
      u.username as author_name,
      u.avatar_url as author_avatar,
      COUNT(DISTINCT pl.user_id) as likes_count
    FROM presets p
    JOIN users u ON p.author_id = u.id
    LEFT JOIN preset_likes pl ON p.id = pl.preset_id
    ${category ? 'WHERE p.category = ?' : ''}
    GROUP BY p.id
    ORDER BY p.created_at DESC
    LIMIT ? OFFSET ?
  `;
  
  const params = category 
    ? [category, limit, (page - 1) * limit]
    : [limit, (page - 1) * limit];
    
  const { results } = await env.DB.prepare(query).bind(...params).all();
  
  return new Response(JSON.stringify({
    data: results,
    page,
    total: results.length
  }), {
    headers: { 'Content-Type': 'application/json' }
  });
});

// 上传预设
router.post('/api/v1/presets', async (request, env) => {
  // JWT 验证
  const token = request.headers.get('Authorization')?.replace('Bearer ', '');
  if (!token) {
    return new Response('Unauthorized', { status: 401 });
  }
  
  const user = await verifyToken(token, env.JWT_SECRET);
  const data = await request.json();
  
  // 插入数据库
  const id = crypto.randomUUID();
  await env.DB.prepare(`
    INSERT INTO presets (id, name, description, category, author_id, config)
    VALUES (?, ?, ?, ?, ?, ?)
  `).bind(
    id,
    data.name,
    data.description,
    data.category,
    user.id,
    JSON.stringify(data.config)
  ).run();
  
  return new Response(JSON.stringify({ id }), {
    status: 201,
    headers: { 'Content-Type': 'application/json' }
  });
});

export default {
  fetch: router.handle
};
```

### 9.2 Tauri 前端集成示例

```typescript
// src/api/workshop.ts
import { fetch } from '@tauri-apps/plugin-http';

const API_BASE = 'https://api.filtermanage.com/v1';

export async function getPresets(params: {
  page?: number;
  category?: string;
  tags?: string[];
  sort?: 'latest' | 'popular';
}) {
  const searchParams = new URLSearchParams({
    page: String(params.page || 1),
    ...(params.category && { category: params.category }),
    ...(params.sort && { sort: params.sort }),
  });
  
  if (params.tags) {
    params.tags.forEach(tag => searchParams.append('tags[]', tag));
  }
  
  const response = await fetch(`${API_BASE}/presets?${searchParams}`);
  return response.json();
}

export async function downloadPreset(id: string, localName: string) {
  const response = await fetch(`${API_BASE}/presets/${id}`);
  const preset = await response.json();
  
  // 保存到本地
  await invoke('save_config', {
    config: {
      name: localName,
      ...preset.config
    }
  });
  
  // 记录下载
  await fetch(`${API_BASE}/presets/${id}/download`, { method: 'POST' });
}
```

## 十、成本与维护

### 初期成本（前 3 个月）
- 云服务：$0-15/月（Cloudflare 免费额度）
- 域名：$10/年
- **总计：≈ $10-50**

### 维护工作量
- 内容审核：1-2 小时/周
- 用户支持：2-3 小时/周
- 功能迭代：按需投入

## 总结

云端工坊功能将为 Filter Manage 带来：
1. ✅ **社区粘性**：用户互相分享，形成生态
2. ✅ **降低门槛**：新用户直接下载优质预设
3. ✅ **品牌传播**：优质预设在社交媒体传播
4. ✅ **用户增长**：工坊内容吸引新用户

推荐从 **方案 A（无服务器架构）** 开始，快速上线 MVP，验证市场需求后再考虑优化和扩展。
```
