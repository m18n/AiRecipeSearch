<template>
  <div class="login-wrapper">
    <div class="login-card">
      <h1 class="login-title">Set Password</h1>
      <p class="login-subtitle">Create a new password for your account</p>

      <div v-if="!token" class="general-error">
        Invalid or missing token. Please use the link from your invitation email.
      </div>

      <form v-else class="login-form" @submit.prevent="handleSubmit">
        <div class="field">
          <label for="password">New Password</label>
          <input
            id="password"
            v-model="fields.password"
            type="password"
            placeholder="Enter new password"
            autocomplete="new-password"
            :class="{ 'input-error': fieldErrors.password }"
            :disabled="isLoading || success"
          />
          <span v-if="fieldErrors.password" class="field-error">
            {{ fieldErrors.password }}
          </span>
        </div>

        <div class="field">
          <label for="confirm">Confirm Password</label>
          <input
            id="confirm"
            v-model="fields.confirm"
            type="password"
            placeholder="Repeat new password"
            autocomplete="new-password"
            :class="{ 'input-error': fieldErrors.confirm }"
            :disabled="isLoading || success"
          />
          <span v-if="fieldErrors.confirm" class="field-error">
            {{ fieldErrors.confirm }}
          </span>
        </div>

        <div v-if="generalError" class="general-error">
          {{ generalError }}
        </div>

        <div v-if="success" class="general-success">
          Password set successfully. Redirecting to login…
        </div>

        <button type="submit" class="btn-submit" :disabled="isLoading || success">
          <span v-if="isLoading" class="spinner" aria-hidden="true" />
          <span>{{ isLoading ? 'Saving…' : 'Set Password' }}</span>
        </button>
      </form>
    </div>
  </div>
</template>

<script setup lang="ts">
import { reactive, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { setPassword as apiSetPassword } from '@/api/auth'

const route = useRoute()
const router = useRouter()

const token = ref<string | null>(
  typeof route.query.token === 'string' ? route.query.token : null,
)

const fields = reactive({ password: '', confirm: '' })
const isLoading = ref(false)
const success = ref(false)
const generalError = ref<string | null>(null)
const fieldErrors = reactive<Record<string, string>>({})

function clearErrors() {
  generalError.value = null
  delete fieldErrors.password
  delete fieldErrors.confirm
}

function validateLocally(): boolean {
  if (fields.password !== fields.confirm) {
    fieldErrors.confirm = 'Passwords do not match'
    return false
  }
  return true
}

async function handleSubmit() {
  clearErrors()

  if (!validateLocally()) return

  isLoading.value = true

  try {
    await apiSetPassword({ token: token.value!, password: fields.password })
    success.value = true
    setTimeout(() => router.push('/login'), 2000)
  } catch (err: any) {
    if (err?.response?.status === 422) {
      const details = err.response.data?.details ?? {}
      for (const [field, message] of Object.entries(details)) {
        fieldErrors[field] = message as string
      }
    } else if (err?.response?.status === 400) {
      generalError.value =
        err.response.data?.message ?? 'Invalid or expired token.'
    } else {
      generalError.value = 'Something went wrong. Please try again.'
    }
  } finally {
    isLoading.value = false
  }
}
</script>
<style scoped>
.login-wrapper {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  background-color: var(--color-bg, #f5f5f5);
  padding: 1rem;
}
.login-card {
  width: 100%;
  max-width: 400px;
  background: var(--color-surface, #ffffff);
  border-radius: 12px;
  padding: 2.5rem 2rem;
  box-shadow: 0 4px 24px rgba(0, 0, 0, 0.08);
}


.login-title {
  font-size: 1.75rem;
  font-weight: 700;
  margin: 0 0 0.25rem;
  color: var(--color-text-primary, #1a1a1a);
}

.login-subtitle {
  font-size: 0.95rem;
  color: var(--color-text-secondary, #666);
  margin: 0 0 2rem;
}

.login-form {
  display: flex;
  flex-direction: column;
  gap: 1.25rem;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 0.375rem;
}

.field label {
  font-size: 0.875rem;
  font-weight: 600;
  color: var(--color-text-primary, #1a1a1a);
}

.field input {
  padding: 0.625rem 0.875rem;
  min-height: 48px;
  border: 1.5px solid var(--color-border, #d1d5db);
  border-radius: 8px;
  font-size: 1rem;
  color: var(--color-text-primary, #1a1a1a);
  background: var(--color-input-bg, #fafafa);
  transition: border-color 0.2s;
  outline: none;
}

.field input:focus {
  border-color: var(--color-primary, #4f46e5);
  background: #fff;
}

.field input:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.input-error {
  border-color: var(--color-error, #ef4444) !important;
}

.field-error {
  font-size: 0.8rem;
  color: var(--color-error, #ef4444);
}

.general-error {
  padding: 0.75rem 1rem;
  background: var(--color-error-bg, #fef2f2);
  border: 1px solid var(--color-error, #ef4444);
  border-radius: 8px;
  font-size: 0.875rem;
  color: var(--color-error, #ef4444);
}

.general-success {
  padding: 0.75rem 1rem;
  background: var(--color-success-bg, #f0fdf4);
  border: 1px solid var(--color-success, #22c55e);
  border-radius: 8px;
  font-size: 0.875rem;
  color: var(--color-success, #16a34a);
}

.btn-submit {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.5rem;
  min-height: 48px;
  padding: 0.75rem;
  border: none;
  border-radius: 8px;
  background: var(--color-primary, #4f46e5);
  color: #fff;
  font-size: 1rem;
  font-weight: 600;
  cursor: pointer;
  transition: background 0.2s, opacity 0.2s;
  margin-top: 0.25rem;
}

.btn-submit:hover:not(:disabled) {
  background: var(--color-primary-hover, #4338ca);
}

.btn-submit:disabled {
  opacity: 0.65;
  cursor: not-allowed;
}

.spinner {
  width: 1rem;
  height: 1rem;
  border: 2px solid rgba(255, 255, 255, 0.4);
  border-top-color: #fff;
  border-radius: 50%;
  animation: spin 0.7s linear infinite;
  flex-shrink: 0;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}
@media (max-width: 480px) {
  .login-wrapper {
    align-items: flex-start;
    padding-top: 2rem;
  }

  .login-card {
    padding: 2rem 1.25rem;
    border-radius: 8px;
    box-shadow: none;
    border: 1px solid #e5e7eb;
  }

  .login-title {
    font-size: 1.5rem;
  }

  .btn-submit {
    font-size: 0.95rem;
  }
}
</style>