<template>
  <div class="theme-compare">
    <div class="compare-container" ref="containerRef">
      <img src="/preview/dark-mode.png" alt="暗色模式" class="compare-img" />
      <div class="light-overlay" :style="{ clipPath: `inset(0 ${100 - sliderPosition}% 0 0)` }">
        <img src="/preview/light-mode.png" alt="亮色模式" class="compare-img light-img" />
      </div>
      <div class="slider-line" :style="{ left: sliderPosition + '%' }" @mousedown="startDrag">
        <div class="slider-handle">
          <span class="slider-icon">‹ ›</span>
        </div>
      </div>
      <div class="label-left">暗色</div>
      <div class="label-right">亮色</div>
    </div>
    <p class="compare-hint">← 拖动滑块对比亮色/暗色模式 →</p>
  </div>
</template>

<script setup>
import { ref, onMounted, onUnmounted } from 'vue'

const containerRef = ref(null)
const sliderPosition = ref(50)
const isDragging = ref(false)

const startDrag = () => {
  isDragging.value = true
}

const onDrag = (e) => {
  if (!isDragging.value || !containerRef.value) return
  const rect = containerRef.value.getBoundingClientRect()
  const x = e.clientX - rect.left
  const percentage = Math.max(0, Math.min(100, (x / rect.width) * 100))
  sliderPosition.value = percentage
}

const stopDrag = () => {
  isDragging.value = false
}

onMounted(() => {
  document.addEventListener('mousemove', onDrag)
  document.addEventListener('mouseup', stopDrag)
})

onUnmounted(() => {
  document.removeEventListener('mousemove', onDrag)
  document.removeEventListener('mouseup', stopDrag)
})
</script>

<style scoped>
.theme-compare {
  margin: 2rem 0;
}

.compare-container {
  position: relative;
  border-radius: 12px;
  overflow: hidden;
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.15);
  cursor: col-resize;
  user-select: none;
}

.compare-img {
  width: 100%;
  display: block;
}

.light-overlay {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
}

.light-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.slider-line {
  position: absolute;
  top: 0;
  bottom: 0;
  width: 3px;
  background: white;
  z-index: 10;
  cursor: col-resize;
  box-shadow: 0 0 10px rgba(0, 0, 0, 0.3);
}

.slider-handle {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  width: 48px;
  height: 48px;
  background: white;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow: 0 2px 12px rgba(0, 0, 0, 0.25);
  cursor: grab;
}

.slider-handle:active {
  cursor: grabbing;
}

.slider-icon {
  font-size: 20px;
  font-weight: bold;
  color: #374151;
  letter-spacing: -2px;
}

.label-left,
.label-right {
  position: absolute;
  bottom: 16px;
  padding: 6px 12px;
  border-radius: 6px;
  font-size: 12px;
  font-weight: 600;
  z-index: 5;
}

.label-left {
  left: 16px;
  background: rgba(255, 255, 255, 0.9);
  color: #1a1a1a;
}

.label-right {
  right: 16px;
  background: rgba(0, 0, 0, 0.7);
  color: #ffffff;
}

.compare-hint {
  text-align: center;
  margin-top: 12px;
  font-size: 13px;
  color: #6b7280;
}
</style>
