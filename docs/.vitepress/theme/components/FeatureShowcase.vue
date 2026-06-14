<template>
  <div class="feature-showcase">
    <div class="showcase-container">
      <Swiper
        :modules="[Navigation, Pagination, Autoplay, EffectFade]"
        :slides-per-view="1"
        :loop="true"
        :effect="'fade'"
        :fade-effect="{ crossFade: true }"
        :speed="1000"
        :autoplay="autoPlay ? { delay: interval, disableOnInteraction: false } : false"
        :navigation="{ prevEl: '.showcase-prev', nextEl: '.showcase-next' }"
        @swiper="onSwiper"
        @slide-change="onSlideChange"
        class="showcase-swiper"
      >
        <SwiperSlide v-for="(slide, index) in slides" :key="index">
          <img 
            :src="slide.image" 
            :alt="slide.title"
            class="slide-image"
            loading="lazy"
          />
        </SwiperSlide>
      </Swiper>
      
      <!-- 自定义箭头 - 放在 Swiper 外部 -->
      <div class="showcase-prev" @click="slidePrev">
        <svg width="20" height="20" viewBox="0 0 20 20" fill="none">
          <path d="M12 4L6 10L12 16" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
      </div>
      <div class="showcase-next" @click="slideNext">
        <svg width="20" height="20" viewBox="0 0 20 20" fill="none">
          <path d="M8 4L14 10L8 16" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
      </div>
      
      <!-- 浮动内容层 -->
      <div class="showcase-content">
        <Transition name="content-fade" mode="out-in">
          <div :key="activeIndex" class="content-inner">
            <span class="slide-badge">{{ activeIndex + 1 }} / {{ slides.length }}</span>
            <h3 class="slide-title">{{ slides[activeIndex].title }}</h3>
            <p class="slide-desc">{{ slides[activeIndex].description }}</p>
          </div>
        </Transition>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref } from 'vue'
import { Swiper, SwiperSlide } from 'swiper/vue'
import { Navigation, Autoplay, EffectFade } from 'swiper/modules'
import 'swiper/css'
import 'swiper/css/navigation'
import 'swiper/css/pagination'
import 'swiper/css/effect-fade'

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

const activeIndex = ref(0)
let swiperInstance = null

function onSwiper(swiper) {
  swiperInstance = swiper
}

function onSlideChange() {
  if (swiperInstance) {
    activeIndex.value = swiperInstance.realIndex
  }
}

function slidePrev() {
  if (swiperInstance) {
    swiperInstance.slidePrev()
  }
}

function slideNext() {
  if (swiperInstance) {
    swiperInstance.slideNext()
  }
}
</script>

<style scoped>
.feature-showcase {
  margin: 2rem 0;
}

.showcase-container {
  position: relative;
  border-radius: 12px;
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.15);
}

.showcase-swiper {
  width: 100%;
  border-radius: 12px;
  overflow: hidden;
}

.slide-image {
  width: 100%;
  display: block;
}

/* 浮动内容层 */
.showcase-content {
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  padding: 60px 32px 28px;
  background: linear-gradient(to top, rgba(0, 0, 0, 0.85) 0%, rgba(0, 0, 0, 0.4) 60%, transparent 100%);
  z-index: 10;
  pointer-events: none;
}

.content-inner {
  max-width: 600px;
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

/* 文案动画 - 与 Swiper 淡入淡出同步 */
.content-fade-enter-active {
  transition: all 0.8s cubic-bezier(0.23, 1, 0.32, 1);
  transition-delay: 0.2s;
}

.content-fade-leave-active {
  transition: all 0.6s cubic-bezier(0.23, 1, 0.32, 1);
}

.content-fade-enter-from {
  opacity: 0;
  transform: translateY(20px);
}

.content-fade-leave-to {
  opacity: 0;
  transform: translateY(-10px);
}

/* 自定义箭头样式 */
.showcase-prev,
.showcase-next {
  position: absolute;
  top: 50%;
  transform: translateY(-50%);
  width: 40px;
  height: 48px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.25);
  backdrop-filter: blur(4px);
  border-radius: 8px;
  color: #ffffff;
  z-index: 20;
  cursor: pointer;
  transition: all 0.3s ease;
  opacity: 0;
}

.showcase-container:hover .showcase-prev,
.showcase-container:hover .showcase-next {
  opacity: 1;
}

.showcase-prev:hover,
.showcase-next:hover {
  background: rgba(0, 0, 0, 0.5);
  transform: translateY(-50%) scale(1.05);
}

.showcase-prev {
  left: 12px;
}

.showcase-next {
  right: 12px;
}

/* Swiper 分页器样式 */
:deep(.swiper-pagination) {
  bottom: 16px !important;
  z-index: 20;
}

:deep(.swiper-pagination-bullet) {
  width: 10px;
  height: 10px;
  background: rgba(0, 0, 0, 0.3);
  opacity: 1;
  transition: all 0.3s ease;
  margin: 0 4px !important;
}

:deep(.swiper-pagination-bullet:hover) {
  background: rgba(0, 0, 0, 0.5);
}

:deep(.swiper-pagination-bullet-active) {
  background: rgba(0, 0, 0, 0.7);
  width: 28px;
  border-radius: 5px;
}

@media (max-width: 768px) {
  .showcase-content {
    padding: 40px 20px 20px;
  }
  
  .slide-title {
    font-size: 22px;
  }
  
  .slide-desc {
    font-size: 13px;
  }
  
  :deep(.swiper-button-prev),
  :deep(.swiper-button-next) {
    width: 32px;
    height: 32px;
  }
  
  :deep(.swiper-button-prev) {
    left: 8px;
  }
  
  :deep(.swiper-button-next) {
    right: 8px;
  }
}
</style>
