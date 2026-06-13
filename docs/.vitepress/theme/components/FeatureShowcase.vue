<template>
  <div class="feature-showcase">
    <div class="showcase-content">
      <div class="showcase-image">
        <Transition name="fade" mode="out-in">
          <img 
            :key="currentSlide" 
            :src="slides[currentSlide].image" 
            :alt="slides[currentSlide].title"
            loading="lazy"
          />
        </Transition>
      </div>
      <div class="showcase-info">
        <Transition name="slide-up" mode="out-in">
          <div :key="currentSlide" class="info-content">
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
  max-width: 900px;
  margin: 0 auto;
}

.showcase-content {
  display: flex;
  gap: 40px;
  align-items: center;
}

.showcase-image {
  flex: 1;
  min-width: 0;
  border-radius: 12px;
  overflow: hidden;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.1);
  background: var(--vp-c-bg-soft);
}

.showcase-image img {
  display: block;
  width: 100%;
  height: auto;
  aspect-ratio: 16/10;
  object-fit: cover;
}

.showcase-info {
  flex: 0 0 280px;
  min-height: 180px;
  display: flex;
  flex-direction: column;
  justify-content: center;
}

.info-content {
  margin-bottom: 24px;
}

.slide-badge {
  display: inline-block;
  padding: 2px 10px;
  font-size: 12px;
  font-weight: 500;
  color: var(--vp-c-brand-1);
  background: var(--vp-c-brand-soft);
  border-radius: 12px;
  margin-bottom: 12px;
}

.slide-title {
  font-size: 24px;
  font-weight: 700;
  color: var(--vp-c-text-1);
  margin: 0 0 10px 0;
  line-height: 1.3;
}

.slide-desc {
  font-size: 14px;
  color: var(--vp-c-text-2);
  line-height: 1.7;
  margin: 0;
}

.slide-dots {
  display: flex;
  gap: 8px;
}

.dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  border: none;
  background: var(--vp-c-divider);
  cursor: pointer;
  padding: 0;
  transition: all 0.3s ease;
}

.dot:hover {
  background: var(--vp-c-text-3);
}

.dot.active {
  background: var(--vp-c-brand-1);
  width: 24px;
  border-radius: 4px;
}

/* 动画 */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.3s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

.slide-up-enter-active {
  transition: all 0.3s ease;
}

.slide-up-leave-active {
  transition: all 0.2s ease;
}

.slide-up-enter-from {
  opacity: 0;
  transform: translateY(10px);
}

.slide-up-leave-to {
  opacity: 0;
  transform: translateY(-10px);
}

@media (max-width: 768px) {
  .showcase-content {
    flex-direction: column;
    gap: 24px;
  }
  
  .showcase-info {
    flex: 1;
    min-height: auto;
    text-align: center;
  }
  
  .slide-dots {
    justify-content: center;
  }
}
</style>
