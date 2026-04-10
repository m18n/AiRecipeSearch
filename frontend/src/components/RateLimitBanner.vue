<template>
  <div class="rate-limit-banner" role="alert">
    <div class="rate-limit-banner__icon">⚠️</div>
    <div class="rate-limit-banner__body">
      <p class="rate-limit-banner__message">
        {{ message }}
        Please try again in <strong>{{ retryAfterMinutes }} minute{{ retryAfterMinutes !== 1 ? 's' : '' }}</strong>.
      </p>
    </div>
    <button
      class="rate-limit-banner__dismiss"
      aria-label="Dismiss"
      @click="emit('dismiss')"
    >
      ✕
    </button>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  message: string
  retryAfterMinutes: number
}>()

const emit = defineEmits<{
  (e: 'dismiss'): void
}>()
</script>

<style scoped>
.rate-limit-banner {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  padding: 14px 16px;
  border: 1px solid #f5a623;
  border-radius: 8px;
  background-color: #fff8ed;
  color: #7a4f00;
}

.rate-limit-banner__icon {
  font-size: 1.25rem;
  flex-shrink: 0;
  line-height: 1.4;
}

.rate-limit-banner__body {
  flex: 1;
  min-width: 0;
}

.rate-limit-banner__message {
  margin: 0;
  font-size: 0.95rem;
  line-height: 1.5;
}

.rate-limit-banner__dismiss {
  flex-shrink: 0;
  background: none;
  border: none;
  cursor: pointer;
  color: #7a4f00;
  font-size: 1rem;
  line-height: 1;
  padding: 2px 4px;
  border-radius: 4px;
  transition: background-color 0.15s ease;
}

.rate-limit-banner__dismiss:hover { background-color: #f5dba0; }

@media (max-width: 480px) {
  .rate-limit-banner {
    padding: 12px;
    gap: 8px;
  }

  .rate-limit-banner__message {
    font-size: 0.875rem;
  }
}
</style>