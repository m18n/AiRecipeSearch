<template>
  <div class="search-cost-badge">
    <span class="search-cost-badge__label">This search cost:</span>
    <span class="search-cost-badge__value">${{ formattedCost }}</span>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{
  costUsd: number
}>()

const formattedCost = computed(() => props.costUsd.toFixed(6))
</script>

<style scoped>
.search-cost-badge {
  display: inline-flex;
  align-items: center;
  gap: 0.375rem;
  font-size: 0.8125rem;
  color: #6b7280;
  flex-wrap: wrap;
}

.search-cost-badge__label { font-weight: 500; }

.search-cost-badge__value {
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  color: #374151;
}

@media (max-width: 480px) {
  .search-cost-badge {
    font-size: 0.75rem;
  }
}
</style>