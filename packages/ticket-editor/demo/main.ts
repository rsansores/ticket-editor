import { createApp } from 'vue'
import { createI18n } from 'vue-i18n'
import Demo from './Demo.vue'

// The host app owns the vue-i18n instance; the editor follows its locale.
const i18n = createI18n({ legacy: false, locale: 'en', fallbackLocale: 'en', messages: {} })

createApp(Demo).use(i18n).mount('#app')
