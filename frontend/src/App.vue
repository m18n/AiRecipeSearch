<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { useAuthStore } from '@/stores/auth'

const router = useRouter()
const authStore = useAuthStore()
const menuOpen = ref(false)

async function handleLogout() {
  menuOpen.value = false
  await authStore.logout()
  router.push('/login')
}
</script>

<template>
  <div id="app">
    <nav v-if="authStore.isAuthenticated" class="navbar">
      <span class="navbar__brand">🍳 RecipeApp</span>

    
      <button
        class="navbar__burger"
        :aria-expanded="menuOpen"
        aria-label="Toggle menu"
        @click="menuOpen = !menuOpen"
      >
        <span /><span /><span />
      </button>

      <div :class="['navbar__links', { 'navbar__links--open': menuOpen }]">
        <RouterLink to="/search"                        class="navbar__link" @click="menuOpen = false">Search</RouterLink>
        <RouterLink to="/cooking-profile/global"        class="navbar__link" @click="menuOpen = false">My Global Preferences</RouterLink>
        <RouterLink to="/cooking-profile/ingredients"   class="navbar__link" @click="menuOpen = false">My Ingredients</RouterLink>
        <RouterLink to="/cooking-profile/kitchen-tools" class="navbar__link" @click="menuOpen = false">My Kitchen Tools</RouterLink>
        <button class="navbar__logout" @click="handleLogout">Logout</button>
      </div>
    </nav>

    <main :class="['main-content', { 'main-content--full': !authStore.isAuthenticated }]">
      <RouterView />
    </main>
  </div>
</template>

<style scoped>
.navbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  padding: 0 1.5rem;
  min-height: 56px;
  background-color: #1e1e2e;
  border-bottom: 1px solid #2e2e3e;
  position: sticky;
  top: 0;
  z-index: 100;
}

.navbar__brand {
  font-size: 1.05rem;
  font-weight: 700;
  color: #cdd6f4;
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
}

.navbar__burger span {
  display: block;
  width: 22px;
  height: 2px;
  background: #cdd6f4;
  border-radius: 2px;
  transition: opacity 0.2s;
}


.navbar__links {
  display: flex;
  align-items: center;
  gap: 1.25rem;
}

.navbar__link {
  color: #cdd6f4;
  text-decoration: none;
  font-size: 0.9rem;
  padding: 0.25rem 0;
  border-bottom: 2px solid transparent;
  transition: border-color 0.2s, color 0.2s;
  white-space: nowrap;
}

.navbar__link:hover { color: #89b4fa; }
.navbar__link.router-link-active {
  color: #89b4fa;
  border-bottom-color: #89b4fa;
}

.navbar__logout {
  background: transparent;
  border: 1px solid #f38ba8;
  color: #f38ba8;
  padding: 0.35rem 0.9rem;
  border-radius: 6px;
  cursor: pointer;
  font-size: 0.875rem;
  transition: background-color 0.2s, color 0.2s;
  white-space: nowrap;
}

.navbar__logout:hover {
  background-color: #f38ba8;
  color: #1e1e2e;
}

.main-content {
  flex: 1;
  padding: 2rem 1.5rem;
}

.main-content--full {
  padding: 0;
  display: flex;
  flex-direction: column;
}


@media (max-width: 768px) {
  .navbar {
    padding: 0 1rem;
  }

  .navbar__burger {
    display: flex;
  }

  .navbar__links {
    display: none;
    width: 100%;
    flex-direction: column;
    align-items: flex-start;
    gap: 0;
    padding: 0.75rem 0 1rem;
  }

  .navbar__links--open {
    display: flex;
  }

  .navbar__link {
    width: 100%;
    padding: 0.6rem 0;
    border-bottom: 1px solid #313244;
    font-size: 0.95rem;
  }

  .navbar__link:last-of-type {
    border-bottom: none;
  }

  .navbar__logout {
    margin-top: 0.75rem;
    width: 100%;
    text-align: center;
  }

  .main-content {
    padding: 1rem;
  }
}
</style>