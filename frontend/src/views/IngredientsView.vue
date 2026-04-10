<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import {
  getIngredients,
  addIngredient,
  deleteIngredient,
  updateIngredientFillPercentage,
  importIngredientsFromCsv,
} from '@/api/userCookingProfile'
import { useIngredientsStore } from '@/stores/ingredients'
import RateLimitBanner from '@/components/RateLimitBanner.vue'
import { watch } from 'vue'

interface Ingredient {
  id: number
  name: string
  fill_percentage: number
  photo_link: string | null
}


const ingredients = ref<Ingredient[]>([])
const listLoading = ref(false)
const listError = ref<string | null>(null)

async function loadIngredients() {
  listLoading.value = true
  listError.value = null
  try {
    ingredients.value = await getIngredients()
  } catch {
    listError.value = 'Failed to load ingredients.'
  } finally {
    listLoading.value = false
  }
}

onMounted(loadIngredients)


const newName = ref('')
const newFillPercentage = ref(1.0)
const newPhotoLink = ref('')
const addLoading = ref(false)
const addError = ref<string | null>(null)

async function handleAdd() {
  if (!newName.value.trim()) return
  addLoading.value = true
  addError.value = null
  try {
    const created = await addIngredient({
      name: newName.value.trim(),
      fill_percentage: newFillPercentage.value,
      photo_link: newPhotoLink.value.trim() || null,
    })
    ingredients.value.push(created)
    newName.value = ''
    newFillPercentage.value = 1.0
    newPhotoLink.value = ''
  } catch {
    addError.value = 'Failed to add ingredient.'
  } finally {
    addLoading.value = false
  }
}


const deleteLoadingId = ref<number | null>(null)

async function handleDelete(id: number) {
  deleteLoadingId.value = id
  try {
    await deleteIngredient(id)
    ingredients.value = ingredients.value.filter((i) => i.id !== id)
  } catch {

  } finally {
    deleteLoadingId.value = null
  }
}


const fillUpdateLoadingId = ref<number | null>(null)

async function handleFillChange(ingredient: Ingredient, newValue: number) {
  fillUpdateLoadingId.value = ingredient.id
  try {
    const updated = await updateIngredientFillPercentage(ingredient.id, newValue)
    const idx = ingredients.value.findIndex((i) => i.id === ingredient.id)
    if (idx !== -1) ingredients.value[idx] = updated
  } catch {

  } finally {
    fillUpdateLoadingId.value = null
  }
}

function formatFill(value: number) {
  return `${Math.round(value * 100)}%`
}


const ingredientsStore = useIngredientsStore()

const selectedFile = ref<File | null>(null)
const fileInputRef = ref<HTMLInputElement | null>(null)

const importDisabled = computed(
  () => !selectedFile.value || ingredientsStore.isImporting || !!ingredientsStore.rateLimitInfo,
)
const fileInputLocked = computed(
  () => ingredientsStore.isImporting || !!ingredientsStore.rateLimitInfo,
)

function handleFileChange(event: Event) {
  const target = event.target as HTMLInputElement
  selectedFile.value = target.files?.[0] ?? null
}

async function handleImport() {
  if (!selectedFile.value) return
  await ingredientsStore.startImport(selectedFile.value)

  if (ingredientsStore.isImporting) {
    watchImportCompletion()
  }
}




function watchImportCompletion() {
  const stop = watch(
    () => ingredientsStore.isImporting,
    (importing) => {
      if (!importing && ingredientsStore.importResult) {
        loadIngredients()
        selectedFile.value = null
        if (fileInputRef.value) fileInputRef.value.value = ''
        stop()
      }
    },
  )
  return stop
}

function handleDismissRateLimit() {
  ingredientsStore.clearRateLimit()
}
</script>

<template>
  <div class="ingredients-view">
    <h1>My Ingredients</h1>

    <section class="section">
      <h2>Your Ingredients</h2>

      <p v-if="listLoading" class="status-text">Loading ingredients…</p>
      <p v-else-if="listError" class="error-text">{{ listError }}</p>
      <p v-else-if="ingredients.length === 0" class="status-text">
        No ingredients yet. Add some below!
      </p>

      <ul v-else class="ingredient-list">
        <li v-for="ingredient in ingredients" :key="ingredient.id" class="ingredient-item">
          <div class="ingredient-info">
            <img
              v-if="ingredient.photo_link"
              :src="ingredient.photo_link"
              :alt="ingredient.name"
              class="ingredient-photo"
            />
            <div class="ingredient-details">
              <span class="ingredient-name">{{ ingredient.name }}</span>
              <span class="ingredient-fill">{{ formatFill(ingredient.fill_percentage) }}</span>
            </div>
          </div>

          <div class="ingredient-controls">
            <label class="fill-label">
              Fill:
              <input
                type="range"
                min="0"
                max="1"
                step="0.01"
                :value="ingredient.fill_percentage"
                :disabled="fillUpdateLoadingId === ingredient.id"
                class="fill-slider"
                @change="
                  handleFillChange(ingredient, +($event.target as HTMLInputElement).value)
                "
              />
              <span class="fill-value">{{ formatFill(ingredient.fill_percentage) }}</span>
            </label>

            <button
              class="btn btn-danger btn-sm"
              :disabled="deleteLoadingId === ingredient.id"
              @click="handleDelete(ingredient.id)"
            >
              {{ deleteLoadingId === ingredient.id ? 'Deleting…' : 'Delete' }}
            </button>
          </div>
        </li>
      </ul>
    </section>

    <section class="section">
      <h2>Add Ingredient</h2>

      <form class="add-form" @submit.prevent="handleAdd">
        <div class="form-group">
          <label for="ing-name">Name <span class="required">*</span></label>
          <input
            id="ing-name"
            v-model="newName"
            type="text"
            placeholder="e.g. Flour"
            required
            class="form-input"
          />
        </div>

        <div class="form-group">
          <label for="ing-fill">Fill percentage</label>
          <div class="fill-row">
            <input
              id="ing-fill"
              v-model.number="newFillPercentage"
              type="range"
              min="0"
              max="1"
              step="0.01"
              class="fill-slider"
            />
            <span class="fill-value">{{ formatFill(newFillPercentage) }}</span>
          </div>
        </div>

        <div class="form-group">
          <label for="ing-photo">Photo URL <span class="optional">(optional)</span></label>
          <input
            id="ing-photo"
            v-model="newPhotoLink"
            type="url"
            placeholder="https://example.com/image.jpg"
            class="form-input"
          />
        </div>

        <p v-if="addError" class="error-text">{{ addError }}</p>

        <button type="submit" class="btn btn-primary" :disabled="addLoading || !newName.trim()">
          {{ addLoading ? 'Adding…' : 'Add Ingredient' }}
        </button>
      </form>
    </section>

    <section class="section">
      <h2>Import from CSV</h2>

      <p class="hint">
        Upload a CSV file with a <code>barcode</code> from any app on your phone. Products will
        be looked up automatically.
        <br />
        <strong>Example:</strong>
        <code>barcode<br />012345678905<br />987654321098</code>
      </p>

      <RateLimitBanner
        v-if="ingredientsStore.rateLimitInfo"
        :message="ingredientsStore.rateLimitInfo.message"
        :retry-after-minutes="ingredientsStore.rateLimitInfo.retry_after_minutes"
        @dismiss="handleDismissRateLimit"
      />

      <template v-else>
        <div class="file-row">
          <input
            ref="fileInputRef"
            type="file"
            accept=".csv"
            :disabled="fileInputLocked"
            class="file-input"
            @change="handleFileChange"
          />

          <button
            class="btn btn-primary"
            :disabled="importDisabled"
            @click="handleImport"
          >
            Import
          </button>
        </div>

        <div v-if="ingredientsStore.isImporting" class="import-progress">
          <span class="spinner" aria-hidden="true" />
          Processing barcodes…
          <span class="progress-label">
            {{ ingredientsStore.importProgress }}%
          </span>
        </div>

        <div
          v-else-if="ingredientsStore.importResult"
          class="import-result"
        >
          <p class="result-summary">
            ✅ Successfully imported:
            <strong>{{ ingredientsStore.importResult.imported.length }}</strong> product(s).
          </p>

          <template v-if="ingredientsStore.importResult.failed.length > 0">
            <p class="result-summary">
              ❌ Failed:
              <strong>{{ ingredientsStore.importResult.failed.length }}</strong> barcode(s).
            </p>
            <ul class="failed-list">
              <li
                v-for="failed in ingredientsStore.importResult.failed"
                :key="failed.barcode"
                class="failed-item"
              >
                <code>{{ failed.barcode }}</code> — {{ failed.reason }}
              </li>
            </ul>
          </template>
        </div>

        <p v-else-if="ingredientsStore.error" class="error-text">
          Import failed: {{ ingredientsStore.error }}
        </p>
      </template>
    </section>
  </div>
</template>

<style scoped>
.ingredients-view {
  max-width: 760px;
  margin: 0 auto;
  padding: 1.5rem 1rem 3rem;
}

h1 {
  font-size: 1.75rem;
  margin-bottom: 1.5rem;
}


.section {
  background: #fff;
  border: 1px solid #e5e7eb;
  border-radius: 0.75rem;
  padding: 1.25rem 1.5rem;
  margin-bottom: 1.5rem;
}

.section h2 {
  font-size: 1.15rem;
  margin-bottom: 1rem;
}
.progress-label {
  font-size: 0.8rem;
  color: #6b7280;
  margin-left: 0.25rem;
}

.status-text {
  color: #6b7280;
  font-style: italic;
}

.error-text {
  color: #dc2626;
  margin-top: 0.5rem;
}


.ingredient-list {
  list-style: none;
  padding: 0;
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.ingredient-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  padding: 0.75rem;
  border: 1px solid #f3f4f6;
  border-radius: 0.5rem;
  background: #fafafa;
  flex-wrap: wrap;
  overflow: hidden; 
}


.ingredient-info {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  min-width: 0;
  flex: 1;
}

.ingredient-photo {
  width: 44px;
  height: 44px;
  object-fit: cover;
  border-radius: 0.375rem;
  flex-shrink: 0;
}

.ingredient-details {
  display: flex;
  flex-direction: column;
  min-width: 0;
  overflow: hidden;
}

.ingredient-name {
  font-weight: 600;
  overflow-wrap: break-word; 
  word-break: break-word;
  width: 100%;
}

.ingredient-fill {
  font-size: 0.8rem;
  color: #6b7280;
}

.ingredient-controls {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  flex-shrink: 0;
}


.fill-label {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  font-size: 0.85rem;
  color: #374151;
}

.fill-row {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.fill-slider {
  width: 110px;
  cursor: pointer;
}

.fill-value {
  font-size: 0.8rem;
  color: #6b7280;
  width: 2.5rem;
  text-align: right;
}


.add-form {
  display: flex;
  flex-direction: column;
  gap: 0.875rem;
}

.form-group {
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
}

.form-group label {
  font-size: 0.875rem;
  font-weight: 500;
  color: #374151;
}

.form-input {
  padding: 0.5rem 0.75rem;
  border: 1px solid #d1d5db;
  border-radius: 0.5rem;
  font-size: 0.95rem;
  outline: none;
  transition: border-color 0.15s;
}

.form-input:focus {
  border-color: #6366f1;
}

.required {
  color: #dc2626;
}

.optional {
  color: #9ca3af;
  font-weight: 400;
}


.hint {
  font-size: 0.875rem;
  color: #6b7280;
  margin-bottom: 1rem;
  line-height: 1.6;
}

.hint code {
  background: #f3f4f6;
  padding: 0.1rem 0.3rem;
  border-radius: 0.25rem;
  font-size: 0.8rem;
  display: inline-block;
}

.file-row {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  flex-wrap: wrap;
}

.file-input {
  font-size: 0.875rem;
}

.import-progress {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  margin-top: 0.875rem;
  color: #374151;
  font-size: 0.9rem;
}

.spinner {
  display: inline-block;
  width: 18px;
  height: 18px;
  border: 2px solid #d1d5db;
  border-top-color: #6366f1;
  border-radius: 50%;
  animation: spin 0.7s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.import-result {
  margin-top: 0.875rem;
}

.result-summary {
  font-size: 0.9rem;
  margin-bottom: 0.4rem;
}

.failed-list {
  list-style: none;
  padding: 0;
  margin: 0.5rem 0 0;
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
}

.failed-item {
  font-size: 0.85rem;
  color: #dc2626;
  background: #fef2f2;
  padding: 0.3rem 0.6rem;
  border-radius: 0.375rem;
}


.btn {
  padding: 0.5rem 1rem;
  border: none;
  border-radius: 0.5rem;
  font-size: 0.9rem;
  font-weight: 500;
  cursor: pointer;
  transition: opacity 0.15s, background-color 0.15s;
}

.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.btn-primary {
  background: #6366f1;
  color: #fff;
}

.btn-primary:not(:disabled):hover {
  background: #4f46e5;
}

.btn-danger {
  background: #fee2e2;
  color: #dc2626;
}

.btn-danger:not(:disabled):hover {
  background: #fecaca;
}

.btn-sm {
  padding: 0.35rem 0.7rem;
  font-size: 0.8rem;
}
@media (max-width: 640px) {
  .ingredients-view {
    padding: 1rem 0.75rem 2rem;
  }

  h1 {
    font-size: 1.4rem;
    margin-bottom: 1rem;
  }

  .section {
    padding: 1rem;
  }

  .ingredient-item {
    flex-direction: column;
    align-items: flex-start;
    padding: 1rem;        
    gap: 0.75rem;
  }

  .ingredient-info {
    width: 100%;           
  }

  .ingredient-details {
    width: 0;
    flex: 1;
  }

  .ingredient-controls {
    width: 100%;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.5rem;
  }

  .fill-label {
    width: 100%;
  }

  .fill-slider {
    width: 100%;
  }

  .btn-danger {
    width: 100%;
    text-align: center;
  }

  .file-row {
    flex-direction: column;
    align-items: stretch;
  }

  .file-input {
    width: 100%;
  }

  .btn-primary {
    width: 100%;
    text-align: center;
  }
}
</style>