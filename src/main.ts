import { createApp } from 'vue'
import App from './App.vue'
import './style.css'

// WebKit hält Animationen an, solange das Fenster verdeckt ist. Ein Einblend-Effekt bliebe dann bei
// „unsichtbar“ stehen. Deshalb laufen sie erst, nachdem das Fenster einmal sichtbar war (`data-motion`).
const enableMotionOnceVisible = () => {
  if (document.visibilityState !== 'visible') return
  document.documentElement.dataset.motion = 'on'
  document.removeEventListener('visibilitychange', enableMotionOnceVisible)
}
enableMotionOnceVisible()
document.addEventListener('visibilitychange', enableMotionOnceVisible)

createApp(App).mount('#app')
