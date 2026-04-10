<script setup lang="ts">
import { ref, onMounted } from 'vue'
import {
  getUserCookingProfile,
  updateGlobalPreferences,
  getCountries,
  getLanguages,
} from '@/api/userCookingProfile'
import type { CountryRow, LanguageRow } from '@/api/userCookingProfile'

const isLoading = ref(false)
const isSaving = ref(false)
const loadError = ref<string | null>(null)
const saveSuccess = ref(false)
const saveError = ref<string | null>(null)

const notes = ref('')
const selectedCountryId = ref<number | null>(null)
const countries = ref<CountryRow[]>([])

const selectedLanguageId = ref<number | null>(null)
const languages = ref<LanguageRow[]>([])

onMounted(async () => {
  isLoading.value = true
  loadError.value = null
  try {
    const [profile, countryList, languageList] = await Promise.all([
      getUserCookingProfile(),
      getCountries(),
      getLanguages(),
    ])
    notes.value = profile.global_preferences?.preference ?? ''
    selectedCountryId.value = profile.global_preferences?.country?.id ?? null
    selectedLanguageId.value = profile.global_preferences?.language?.id ?? null
    countries.value = countryList
    languages.value = languageList
  } catch (e: any) {
    loadError.value = e?.message ?? 'Failed to load preferences.'
  } finally {
    isLoading.value = false
  }
})


async function handleSave() {
  isSaving.value = true
  saveSuccess.value = false
  saveError.value = null
  try {
    await updateGlobalPreferences({
      preference: notes.value || null,
      country_of_residence_id: selectedCountryId.value,
      language_id: selectedLanguageId.value,
    })
    saveSuccess.value = true
    setTimeout(() => (saveSuccess.value = false), 3000)
  } catch (e: any) {
    saveError.value = e?.message ?? 'Failed to save preferences.'
  } finally {
    isSaving.value = false
  }
}
</script>

<template>
  <div class="global-preferences">
    <h1 class="page-title">My Global Preferences</h1>
    <p class="page-subtitle">
      Describe your dietary preferences, restrictions, or any other instructions for the AI.
      These settings influence every recipe search you make.
    </p>

    <div v-if="isLoading" class="state-message state-message--loading">
      Loading your preferences...
    </div>

    <div v-else-if="loadError" class="state-message state-message--error">
      {{ loadError }}
    </div>

    <form v-else class="preferences-form" @submit.prevent="handleSave">
  
      <div class="field">
        <label class="field-label" for="country-select">Current location</label>
        <select
          id="country-select"
          v-model="selectedCountryId"
          class="select"
        >
          <option :value="null">— Not specified —</option>
          <option
            v-for="country in countries"
            :key="country.id"
            :value="country.id"
          >
            {{ country.name }}
          </option>
        </select>
      </div>
      <div class="field">
        <label class="field-label" for="language-select">Preferred language</label>
        <select
          id="language-select"
          v-model="selectedLanguageId"
          class="select"
        >
          <option :value="null">— Not specified —</option>
          <option
            v-for="lang in languages"
            :key="lang.id"
            :value="lang.id"
          >
            {{ lang.name }}
          </option>
        </select>
      </div>

      <div class="field">
        <label class="field-label" for="notes">Dietary notes & instructions</label>
        <textarea
          id="notes"
          v-model="notes"
          class="textarea"
          rows="8"
          placeholder="e.g. I'm vegetarian, avoid nuts, prefer Mediterranean cuisine, no spicy food..."
        />
      </div>

      <div v-if="saveSuccess" class="state-message state-message--success">
        ✓ Preferences saved successfully.
      </div>
      <div v-if="saveError" class="state-message state-message--error">
        {{ saveError }}
      </div>

      <div class="form-actions">
        <button type="submit" class="btn btn--primary" :disabled="isSaving">
          <span v-if="isSaving">Saving...</span>
          <span v-else>Save Preferences</span>
        </button>
      </div>
    </form>
  </div>
</template>

<style scoped>
.global-preferences {
  max-width: 720px;
  margin: 0 auto;
  padding: 2rem 1rem;
}


.page-title {
  font-size: 1.75rem;
  font-weight: 700;
  margin-bottom: 0.5rem;
}

.page-subtitle {
  color: #6b7280;
  margin-bottom: 2rem;
}

.preferences-form {
  display: flex;
  flex-direction: column;
  gap: 1.25rem;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
}

.field-label {
  font-size: 0.875rem;
  font-weight: 600;
  color: #374151;
}

.select {
  width: 100%;
  padding: 0.65rem 0.75rem;
  border: 1.5px solid #d1d5db;
  border-radius: 0.5rem;
  font-size: 0.95rem;
  font-family: inherit;
  background: #fff;
  cursor: pointer;
  transition: border-color 0.15s;
  box-sizing: border-box;
}

.select:focus {
  outline: none;
  border-color: #6366f1;
}

.textarea {
  width: 100%;
  padding: 0.75rem;
  border: 1.5px solid #d1d5db;
  border-radius: 0.5rem;
  font-size: 0.95rem;
  font-family: inherit;
  resize: vertical;
  transition: border-color 0.15s;
  box-sizing: border-box;
}

.textarea:focus {
  outline: none;
  border-color: #6366f1;
}

.state-message {
  padding: 0.75rem 1rem;
  border-radius: 0.5rem;
  font-size: 0.9rem;
}

.state-message--loading { color: #6b7280; background: #f3f4f6; }
.state-message--success { color: #065f46; background: #d1fae5; }
.state-message--error   { color: #991b1b; background: #fee2e2; }

.form-actions {
  display: flex;
  justify-content: flex-end;
}

.btn {
  padding: 0.6rem 1.5rem;
  border-radius: 0.5rem;
  font-size: 0.95rem;
  font-weight: 600;
  border: none;
  cursor: pointer;
  transition: background 0.15s, opacity 0.15s;
}

.btn--primary { background: #6366f1; color: #fff; }
.btn--primary:hover:not(:disabled) { background: #4f46e5; }
.btn--primary:disabled { opacity: 0.6; cursor: not-allowed; }
@media (max-width: 640px) {
  .global-preferences {
    padding: 1.25rem 0.75rem;
  }

  .page-title {
    font-size: 1.4rem;
  }

  .page-subtitle {
    font-size: 0.875rem;
    margin-bottom: 1.5rem;
  }

  .btn {
    width: 100%;
    text-align: center;
  }

  .form-actions {
    justify-content: stretch;
  }
}
</style>