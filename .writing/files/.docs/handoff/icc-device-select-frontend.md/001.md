# ICC 滤镜切换 — 传递选定显示器 ID

## 问题

用户选定了某个显示器后，点击 ICC 滤镜切换时，滤镜始终应用到主显示器（第一个），而非用户选定的显示器。

## 原因

后端 `set_icc_profile` 和 `restore_default_icc_profile` 已支持 `device_id: Option<String>` 参数，但 `ProfileList` 组件调用时未传递 `selectedDeviceId`。

## 后端已完成

无需修改。接口签名已正确：

```rust
pub async fn set_icc_profile(profile_path: String, device_id: Option<String>) -> Result<(), String>
pub async fn restore_default_icc_profile(device_id: Option<String>) -> Result<(), String>
```

## 前端需要修改

### 1. `ProfileList` 组件添加 `selectedDeviceId` prop

```tsx
// src/components/ProfileList.tsx

interface ProfileListProps {
  activeProfile: string;
  onProfileSelect: (profile: string) => void;
  showToast: (type: "success" | "error", text: string) => void;
  selectedDeviceId?: string; // ← 新增
}

function ProfileList({ activeProfile, onProfileSelect, showToast, selectedDeviceId }: ProfileListProps) {
```

### 2. `handleProfileSelect` 传递 `deviceId`

```tsx
const handleProfileSelect = async (profile: IccProfile) => {
  try {
    await invoke("set_icc_profile", {
      profilePath: profile.path,
      deviceId: selectedDeviceId, // ← 新增
    });
    onProfileSelect(profile.name);
  } catch (err) {
    console.error("Failed to set ICC profile:", err);
  }
};
```

### 3. `handleRestoreDefault` 传递 `deviceId`

```tsx
const handleRestoreDefault = async () => {
  try {
    await invoke("restore_default_icc_profile", {
      deviceId: selectedDeviceId, // ← 新增
    });
    onProfileSelect("Default");
  } catch (err) {
    console.error("Failed to restore default ICC profile:", err);
  }
};
```

### 4. `App.tsx` 传递 prop

```tsx
<ProfileList
  activeProfile={activeProfile}
  onProfileSelect={handleProfileChange}
  showToast={showToast}
  selectedDeviceId={selectedDeviceId}  // ← 新增
/>
```

### 5. `App.tsx` 中 config 加载时也传 `deviceId`

```tsx
// 约第 170 行，加载配置时应用 ICC
if (match) await invoke("set_icc_profile", { profilePath: match.path, deviceId: selectedDeviceId });
```

## 行为说明

- `deviceId` 为 `undefined` 时，后端 fallback 到主显示器（保持向后兼容）
- `deviceId` 格式为 `\\.\DISPLAY1`、`\\.\DISPLAY2` 等（即 `DisplayMonitor.device_id` 字段）
