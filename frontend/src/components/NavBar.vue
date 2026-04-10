<script setup lang="ts">
import { ref } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useAuthStore } from '@/stores/auth'

const router = useRouter()
const route = useRoute()
const authStore = useAuthStore()
const menuOpen = ref(false)

const navLinks = [
  { label: 'Search', to: '/search' },
  { label: 'My Global Preferences', to: '/cooking-profile/global' },
  { label: 'My Ingredients', to: '/cooking-profile/ingredients' },
  { label: 'My Kitchen Tools', to: '/cooking-profile/kitchen-tools' },
]

async function handleLogout() {
  menuOpen.value = false
  await authStore.logout()
  router.push('/login')
}
</script>

<template>
  <nav class="navbar">
    <div class="navbar__brand">🍳 RecipeApp</div>

    <button
      class="navbar__burger"
      :aria-expanded="menuOpen"
      aria-label="Toggle menu"
      @click="menuOpen = !menuOpen"
    >
      <span /><span /><span />
    </button>

    <ul :class="['navbar__links', { 'navbar__links--open': menuOpen }]">
      <li v-for="link in navLinks" :key="link.to">
        <router-link
          :to="link.to"
          class="navbar__link"
          :class="{ 'navbar__link--active': route.path === link.to }"
          @click="menuOpen = false"
        >
          {{ link.label }}
        </router-link>
      </li>
    </ul>

    <button class="navbar__logout" @click="handleLogout">Logout</button>
  </nav>
</template>

<style scoped>
.navbar {
  display: flex;
  align-items: center;
  gap: 1rem;
  padding: 0 1.5rem;
  height: 56px;
  background-color: #1e1e2e;
  border-bottom: 1px solid #313244;
  position: sticky;
  top: 0;
  z-index: 100;
}

.navbar__brand {
  font-size: 1.1rem;
  font-weight: 700;
  color: #cdd6f4;
  margin-right: auto;
  white-space: nowrap;
}

.navbar__burger {
  display: none;
  flex-direction: column;
  justify-content: center;
  gap: 5px;
  background: none;
  border: none;
  cursor: pointer;
  padding: 0.25rem;
  margin-left: auto;
}

.navbar__burger span {
  display: block;
  width: 22px;
  height: 2px;
  background: #cdd6f4;
  border-radius: 2px;
}

.navbar__links {
  display: flex;
  align-items: center;
  gap: 0.25rem;
  list-style: none;
  margin: 0;
  padding: 0;
}

.navbar__link {
  display: block;
  padding: 0.4rem 0.75rem;
  border-radius: 6px;
  font-size: 0.875rem;
  color: #a6adc8;
  text-decoration: none;
  transition: background-color 0.15s, color 0.15s;
  white-space: nowrap;
}

.navbar__link:hover { background-color: #313244; color: #cdd6f4; }
.navbar__link--active { background-color: #45475a; color: #cdd6f4; font-weight: 600; }

.navbar__logout {
  padding: 0.4rem 0.9rem;
  border-radius: 6px;
  border: 1px solid #f38ba8;
  background: transparent;
  color: #f38ba8;
  font-size: 0.875rem;
  cursor: pointer;
  transition: background-color 0.15s, color 0.15s;
  white-space: nowrap;
}

.navbar__logout:hover { background-color: #f38ba8; color: #1e1e2e; }


@media (max-width: 768px) {
  .navbar {
    flex-wrap: wrap;
    height: auto;
    padding: 0.75rem 1rem;
  }

  .navbar__burger {
    display: flex;
  }

  .navbar__logout {
    display: none; 
  }

  .navbar__links {
    display: none;
    width: 100%;
    flex-direction: column;
    align-items: stretch;
    gap: 0;
    padding: 0.5rem 0;
  }

  .navbar__links--open {
    display: flex;
  }

  .navbar__links li {
    width: 100%;
  }

  .navbar__link {
    width: 100%;
    border-radius: 6px;
    padding: 0.6rem 0.75rem;
    font-size: 0.95rem;
  }

  
  .navbar__links--open::after {
    content: '';
  }
}
</style>