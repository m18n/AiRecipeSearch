<template>
  <div class="search-view">
    <div class="search-header">
      <h1>Recipe Search</h1>
      <p class="search-subtitle">Enter a dish name or a list of ingredients to find a recipe.</p>
    </div>

    <form class="search-form" @submit.prevent="handleSearch">
      <div class="search-input-wrapper">
        <textarea
          v-model="query"
          class="search-input"
          placeholder="e.g. pasta carbonara, or: chicken, garlic, lemon, rosemary"
          rows="3"
          :disabled="isLoading || !!recipesStore.rateLimitInfo"
        />
      </div>

      <div class="appliance-field">
        <label for="appliance-select">Kitchen Appliance</label>

        <div v-if="appliancesLoading" class="appliance-loading">
          Loading appliances…
        </div>

        <div v-else-if="appliances.length === 0" class="appliance-empty">
          No appliances saved yet.
          <router-link to="/cooking-profile/kitchen-tools">Add one here</router-link>
        </div>

        <select
          v-else
          id="appliance-select"
          v-model="selectedApplianceId"
          class="appliance-select"
          :disabled="isLoading || !!recipesStore.rateLimitInfo"
        >
          <option :value="null" disabled>Select an appliance…</option>
          <option
            v-for="appliance in appliances"
            :key="appliance.id"
            :value="appliance.id"
          >
            {{ appliance.name }}
            <template v-if="appliance.description"> — {{ appliance.description }}</template>
          </option>
        </select>
      </div>

      <div class="search-actions">
        <button
          type="submit"
          class="btn btn--primary"
          :disabled="!canSubmit"
        >
          <span v-if="isLoading" class="btn-spinner" />
          <span>{{ submitLabel }}</span>
        </button>
        <span v-if="recipesStore.rateLimitInfo" class="rate-limit-countdown">
          Available in {{ recipesStore.rateLimitInfo.retry_after_minutes }} min
        </span>
      </div>
    </form>
   <RateLimitBanner
    v-if="recipesStore.rateLimitInfo"
    :message="recipesStore.rateLimitInfo.message"
    :retry-after-minutes="recipesStore.rateLimitInfo.retry_after_minutes"
    @dismiss="handleDismissRateLimit"
  />

    <div v-else-if="isLoading" class="status-area">
      <div class="loader-wrapper">
        <div class="spinner" />
        <p class="status-message">{{ statusMessage }}</p>

        <div v-if="recipesStore.jobStatus === 'processing'" class="progress-bar-wrapper">
          <div
            class="progress-bar"
            :style="{ width: recipesStore.jobProgress + '%' }"
          />
          <span class="progress-label">{{ recipesStore.jobProgress }}%</span>
        </div>
      </div>
    </div>

    <div v-else-if="recipesStore.error" class="error-area">
      <p class="error-message">{{ recipesStore.error }}</p>
    </div>

    <div v-else-if="recipesStore.currentRecipe" class="result-area">

      <div class="result-toolbar">
        <button class="btn btn--copy" @click="copyRecipe">
          <svg v-if="!copied" xmlns="http://www.w3.org/2000/svg" width="15" height="15" viewBox="0 0 24 24"
              fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <rect x="9" y="9" width="13" height="13" rx="2" ry="2"/>
            <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
          </svg>
          <svg v-else xmlns="http://www.w3.org/2000/svg" width="15" height="15" viewBox="0 0 24 24"
              fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="20 6 9 17 4 12"/>
          </svg>
          <span>{{ copied ? 'Copied!' : 'Copy recipe' }}</span>
        </button>
      </div>

      <div class="recipe-content markdown-body" v-html="renderedMarkdown" />

      <SearchCostBadge
        v-if="recipesStore.currentRecipeCost !== null"
        :cost-usd="recipesStore.currentRecipeCost"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted } from 'vue'
import { marked } from 'marked'
import { useRecipesStore } from '@/stores/recipes'
import { getAppliances } from '@/api/userCookingProfile'
import RateLimitBanner from '@/components/RateLimitBanner.vue'
import SearchCostBadge from '@/components/SearchCostBadge.vue'

const recipesStore = useRecipesStore()


interface Appliance {
  id: number
  name: string
  description?: string | null
}

const appliances = ref<Appliance[]>([])
const appliancesLoading = ref(false)
const selectedApplianceId = ref<number | null>(null)

onMounted(async () => {
  appliancesLoading.value = true
  try {
    appliances.value = await getAppliances()
    if (appliances.value.length === 1) {
      selectedApplianceId.value = appliances.value[0]?.id ?? null
    }
  } catch {

  } finally {
    appliancesLoading.value = false
  }
})


const query = ref('')


const isLoading = computed(() =>
  ['pending', 'processing'].includes(recipesStore.jobStatus ?? '')
)

const canSubmit = computed(() =>
  query.value.trim().length > 0 &&
  selectedApplianceId.value !== null &&
  !isLoading.value &&
  !recipesStore.rateLimitInfo
)

const statusMessage = computed(() => {
  switch (recipesStore.jobStatus) {
    case 'pending':
      return 'Queuing your search…'
    case 'processing':
      return 'Searching the web and generating your recipe…'
    default:
      return 'Working…'
  }
})

const submitLabel = computed(() => (isLoading.value ? 'Searching…' : 'Search'))


const renderedMarkdown = computed(() => {
  if (!recipesStore.currentRecipe) return ''
  return marked.parse(recipesStore.currentRecipe) as string
})

const copied = ref(false)

async function copyRecipe() {
  if (!recipesStore.currentRecipe) return
  await navigator.clipboard.writeText(recipesStore.currentRecipe)
  copied.value = true
  setTimeout(() => (copied.value = false), 2000)
}

async function handleSearch() {
  if (!canSubmit.value || selectedApplianceId.value === null) return

  await recipesStore.startSearch({
    query: query.value.trim(),
    kitchen_appliances_id: selectedApplianceId.value,
  })
}

function handleDismissRateLimit() {
  recipesStore.clearRateLimit()
}


watch(query, () => {
  if (recipesStore.error) recipesStore.error = null
})
</script>

<style scoped>
.result-toolbar {
  display: flex;
  justify-content: flex-end;
}

.btn--copy {
  background: #f3f4f6;
  color: #374151;
  border: 1px solid #e5e7eb;
  font-size: 0.875rem;
  padding: 0.45rem 1rem;
  gap: 0.4rem;
}

.btn--copy:hover {
  background: #e5e7eb;
}

.search-view {
  max-width: 860px;
  margin: 0 auto;
  padding: 2rem 1rem;
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
}


.search-header {
  text-align: center;
}

.search-header h1 {
  font-size: 2rem;
  font-weight: 700;
  margin-bottom: 0.25rem;
}

.search-subtitle {
  color: #6b7280;
  font-size: 0.95rem;
}


.search-form {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  background: #fff;
  border: 1px solid #e5e7eb;
  border-radius: 12px;
  padding: 1.5rem;
}

.search-input {
  width: 100%;
  resize: vertical;
  padding: 0.75rem 1rem;
  border: 1px solid #d1d5db;
  border-radius: 8px;
  font-size: 1rem;
  line-height: 1.5;
  transition: border-color 0.2s;
  box-sizing: border-box;
}

.search-input:focus {
  outline: none;
  border-color: #6366f1;
  box-shadow: 0 0 0 3px rgb(99 102 241 / 0.15);
}

.search-input:disabled {
  background: #f9fafb;
  color: #9ca3af;
}


.appliance-field {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}

.appliance-field label {
  font-size: 0.8rem;
  font-weight: 600;
  color: #374151;
}

.appliance-select {
  padding: 0.55rem 0.75rem;
  border: 1px solid #d1d5db;
  border-radius: 8px;
  font-size: 0.95rem;
  background: #fff;
  cursor: pointer;
  transition: border-color 0.2s;
}

.appliance-select:focus {
  outline: none;
  border-color: #6366f1;
  box-shadow: 0 0 0 3px rgb(99 102 241 / 0.15);
}

.appliance-select:disabled {
  background: #f9fafb;
  color: #9ca3af;
  cursor: not-allowed;
}

.appliance-loading {
  font-size: 0.875rem;
  color: #9ca3af;
  padding: 0.4rem 0;
}

.appliance-empty {
  font-size: 0.875rem;
  color: #b45309;
  padding: 0.4rem 0;
}

.appliance-empty a {
  color: #6366f1;
  text-decoration: underline;
  margin-left: 0.25rem;
}


.search-actions {
  display: flex;
  align-items: center;
  gap: 1rem;
}

.btn {
  display: inline-flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.625rem 1.5rem;
  border: none;
  border-radius: 8px;
  font-size: 0.95rem;
  font-weight: 600;
  cursor: pointer;
  transition: background 0.2s, opacity 0.2s;
}

.btn--primary {
  background: #6366f1;
  color: #fff;
}

.btn--primary:hover:not(:disabled) {
  background: #4f46e5;
}

.btn:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}

.btn-spinner {
  width: 14px;
  height: 14px;
  border: 2px solid rgb(255 255 255 / 0.4);
  border-top-color: #fff;
  border-radius: 50%;
  animation: spin 0.7s linear infinite;
  flex-shrink: 0;
}

.rate-limit-countdown {
  font-size: 0.85rem;
  color: #b45309;
  font-weight: 500;
}


.status-area {
  display: flex;
  justify-content: center;
  padding: 3rem 1rem;
}

.loader-wrapper {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 1rem;
}

.spinner {
  width: 40px;
  height: 40px;
  border: 3px solid #e5e7eb;
  border-top-color: #6366f1;
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

.status-message {
  color: #6b7280;
  font-size: 0.95rem;
}
.progress-bar-wrapper {
  width: 260px;
  background: #e5e7eb;
  border-radius: 999px;
  height: 8px;
  position: relative;
  margin-top: 0.25rem;
}

.progress-bar {
  height: 100%;
  background: #6366f1;
  border-radius: 999px;
  transition: width 0.4s ease;
}

.progress-label {
  position: absolute;
  right: 0;
  top: 12px;
  font-size: 0.75rem;
  color: #6b7280;
}
@keyframes spin {
  to { transform: rotate(360deg); }
}


.error-area {
  background: #fef2f2;
  border: 1px solid #fecaca;
  border-radius: 10px;
  padding: 1.25rem 1.5rem;
}

.error-message {
  color: #dc2626;
  font-size: 0.95rem;
  margin: 0;
}


.result-area {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.recipe-content {
  background: #fff;
  border: 1px solid #e5e7eb;
  border-radius: 12px;
  padding: 2rem;
  line-height: 1.7;
}


.markdown-body :deep(h1),
.markdown-body :deep(h2),
.markdown-body :deep(h3) {
  font-weight: 700;
  margin-top: 1.4em;
  margin-bottom: 0.5em;
}

.markdown-body :deep(ul),
.markdown-body :deep(ol) {
  padding-left: 1.5rem;
  margin: 0.5rem 0;
}

.markdown-body :deep(table) {
  border-collapse: collapse;
  width: 100%;
  margin: 1rem 0;
  font-size: 0.9rem;
  display: block;
  overflow-x: auto;
  -webkit-overflow-scrolling: touch;
}

.markdown-body :deep(th),
.markdown-body :deep(td) {
  border: 1px solid #e5e7eb;
  padding: 0.5rem 0.75rem;
  text-align: left;
}

.markdown-body :deep(th) {
  background: #f9fafb;
  font-weight: 600;
}

.markdown-body :deep(pre) {
  overflow-x: auto;
  -webkit-overflow-scrolling: touch;
  background: #f3f4f6;
  border-radius: 6px;
  padding: 0.75rem 1rem;
  font-size: 0.85em;
}

.markdown-body :deep(pre) code {
  background: none;
  padding: 0;
  font-size: inherit;
}

.markdown-body :deep(code) {
  background: #f3f4f6;
  padding: 0.1em 0.35em;
  border-radius: 4px;
  font-size: 0.88em;
}

.markdown-body :deep(blockquote) {
  border-left: 3px solid #6366f1;
  margin: 0.75rem 0;
  padding: 0.5rem 1rem;
  color: #6b7280;
}

.markdown-body :deep(img) {
  max-width: 100%;
  height: auto;
  border-radius: 6px;
}

.markdown-body :deep(p),
.markdown-body :deep(li) {
  overflow-wrap: break-word;
  word-break: break-word;
}
@media (max-width: 640px) {
  .search-view {
    padding: 1rem 0.75rem;
    gap: 1rem;
  }

  .search-header h1 {
    font-size: 1.5rem;
  }

  .search-subtitle {
    font-size: 0.875rem;
  }

  .search-form {
    padding: 1rem;
    border-radius: 8px;
  }

  .search-input {
    font-size: 0.95rem;
  }

  .search-actions {
    flex-direction: column;
    align-items: stretch;
    gap: 0.5rem;
  }

  .btn {
    width: 100%;
    justify-content: center;
  }

  .rate-limit-countdown {
    text-align: center;
  }

  .recipe-content {
    padding: 1.25rem 1rem;
    border-radius: 8px;
  }

  .spinner {
    width: 32px;
    height: 32px;
  }
}
</style>