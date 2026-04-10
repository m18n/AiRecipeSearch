import { createRouter, createWebHistory } from 'vue-router'
import { useAuthStore } from '@/stores/auth'

import LoginView from '@/views/LoginView.vue'
import SearchView from '@/views/SearchView.vue'
import GlobalPreferencesView from '@/views/GlobalPreferencesView.vue'
import IngredientsView from '@/views/IngredientsView.vue'
import KitchenToolsView from '@/views/KitchenToolsView.vue'
import SetPasswordView from '@/views/SetPasswordView.vue'

const routes = [
  {
    path: '/login',
    name: 'Login',
    component: LoginView,
    meta: { requiresAuth: false },
  },
  {
    path: '/set-password',
    name: 'set-password',
    component: SetPasswordView,
    meta: { requiresAuth: false },
  },
  {
    path: '/search',
    name: 'Search',
    component: SearchView,
    meta: { requiresAuth: true },
  },
  {
    path: '/cooking-profile/global',
    name: 'GlobalPreferences',
    component: GlobalPreferencesView,
    meta: { requiresAuth: true },
  },
  {
    path: '/cooking-profile/ingredients',
    name: 'Ingredients',
    component: IngredientsView,
    meta: { requiresAuth: true },
  },
  {
    path: '/cooking-profile/kitchen-tools',
    name: 'KitchenTools',
    component: KitchenToolsView,
    meta: { requiresAuth: true },
  },
  {
    path: '/:pathMatch(.*)*',
    redirect: '/search',
  },
]

const router = createRouter({
  history: createWebHistory(),
  routes,
})

router.beforeEach((to, _from, next) => {
  const authStore = useAuthStore()

  if (to.meta.requiresAuth && !authStore.isAuthenticated) {
    next({ name: 'Login', query: { redirect: to.fullPath } })
  } else if (to.name === 'Login' && authStore.isAuthenticated) {
    next({ name: 'Search' })
  } else {
    next()
  }
})

export default router