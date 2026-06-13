<template>
  <div class="feature-showcase">
    <div class="showcase-container">
      <Transition name="fade" mode="out-in">
        <img 
          :key="currentSlide" 
          :src="slides[currentSlide].image" 
          :alt="slides[currentSlide].title"
          class="showcase-image"
          loading="lazy"
        />
      </Transition>
      <div class="showcase-overlay">
        <div class="overlay-content">
          <Transition name="slide-up" mode="out-in">
            <div :key="currentSlide" class="info-block">
              <span class="slide-badge">{{ currentSlide + 1 }} / {{ slides.length }}</span>
              <h3 class="slide-title">{{ slides[currentSlide].title }}</h3>
              <p class="slide-desc">{{ slides[currentSlide].description }}</p>
            </div>
          </Transition>
          <div class="slide-dots">
            <button 
              v-for="(slide, index) in slides" 
              :key="index"
              :class="['dot', { active: currentSlide === index }]"
              @click="goTo(index)"
              :aria-label="slide.title"
            />
          </div>
        </div>
      </div>
    </div>
    <p class="showcase-hint">点击圆点或等待自动切换浏览功能截图</p>
  </div>
</template>

<script setup>
import { ref, onMounted, onUnmounted } from 'vue'

const props = defineProps({
  autoPlay: {
    type: Boolean,
    default: true
  },
  interval: {
    type: Number,
    default: 4000
  }
})

const slides = [
  {
    image: '/preview/dark-mode.png',
    title: '暗色模式',
    description: '深邃优雅的深色背景，适合夜间使用，保护眼睛，减少视觉疲劳'
  },
  {
    image: '/preview/light-mode.png',
    title: '亮色模式',
    description: '清新明亮的白色基调，适合日间使用，精致的阴影和边框效果'
  },
  {
    image: '/preview/settings-general.png',
    title: '常规设置',
    description: '配置应用基本参数，自定义启动行为和窗口显示方式'
  },
  {
    image: '/preview/settings-shortcut.png',
    title: '快捷键设置',
    description: '设置全局快捷键，快速切换配置方案，提升操作效率'
  },
  {
    image: '/preview/settings-display.png',
    title: '显示适配',
    description: '支持多显示器，为每个显示器单独配置 ICC 和颜色设置'
  },
  {
    image: '/preview/settings-process.png',
    title: '进程监听',
    description: '设置进程规则，启动指定程序时自动切换配置方案'
  }
]

const currentSlide = ref(0)
let timer = null

function next() {
  currentSlide.value = (currentSlide.value + 1) % slides.length
}

function goTo(index) {
  currentSlide.value = index
  resetTimer()
}

function resetTimer() {
  if (timer) clearInterval(timer)
  if (props.autoPlay) {
    timer = setInterval(next, props.interval)
  }
}

onMounted(() => {
  resetTimer()
})

onUnmounted(() => {
  if (timer) clearInterval(timer)
})
</script>

<style scoped>
.feature-showcase {
  margin: 2rem 0;
}

.showcase-container {
  position: relative;
  border-radius: 12px;
  overflow: hidden;
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.15);
  cursor: default;
}

.showcase-image {
  width: 100%;
  display: block;
}

.showcase-overlay {
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  padding: 60px 32px 28px;
  background: linear-gradient(to top, rgba(0, 0, 0, 0.85) 0%, rgba(0, 0, 0, 0.4) 60%, transparent 100%);
}

.overlay-content {
  max-width: 600px;
}

.info-block {
  margin-bottom: 16px;
}

.slide-badge {
  display: inline-block;
  padding: 2px 10px;
  font-size: 12px;
  font-weight: 500;
  color: rgba(255, 255, 255, 0.9);
  background: rgba(255, 255, 255, 0.2);
  border-radius: 12px;
  margin-bottom: 10px;
  backdrop-filter: blur(4px);
}

.slide-title {
  font-size: 28px;
  font-weight: 700;
  color: #ffffff;
  margin: 0 0 8px 0;
  line-height: 1.3;
  text-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
}

.slide-desc {
  font-size: 15px;
  color: rgba(255, 255, 255, 0.9);
  line-height: 1.6;
  margin: 0;
  text-shadow: 0 1px 4px rgba(0, 0, 0, 0.3);
}

.slide-dots {
  display: flex;
  gap: 8px;
}

.dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  border: 2px solid rgba(255, 255, 255, 0.6);
  background: transparent;
  cursor: pointer;
  padding: 0;
  transition: all 0.3s ease;
}

.dot:hover {
  background: rgba(255, 255, 255, 0.5);
}

.dot.active {
  background: #ffffff;
  border-color: #ffffff;
  width: 24px;
  border-radius: 4px;
}

.showcase-hint {
  text-align: center;
  margin-top: 12px;
  font-size: 13px;
  color: #6b7280;
}

/* 动画 */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.4s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

.slide-up-enter-active {
  transition: all 0.4s ease;
}

.slide-up-leave-active {
  transition: all 0.2s ease;
}

.slide-up-enter-from {
  opacity: 0;
  transform: translateY(15px);
}

.slide-up-leave-to {
  opacity: 0;
  transform: translateY(-10px);
}

@media (max-width: 768px) {
  .showcase-overlay {
    padding: 40px 20px 20px;
  }
  
  .slide-title {
    font-size: 22px;
  }
  
  .slide-desc {
    font-size: 13px;
  }
}
</style>
