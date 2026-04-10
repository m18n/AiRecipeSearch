import { defineStore } from 'pinia'
import { ref } from 'vue'
import { importIngredientsFromCsv, getImportJobStatus } from '@/api/userCookingProfile'
import type { RateLimitInfo } from '@/types/rateLimit'
import { isRateLimitError } from '@/types/rateLimit'


export interface ImportedIngredient {
  barcode: string
  name: string
  photo_link: string | null
}

export interface FailedBarcode {
  barcode: string
  reason: string
}

interface ImportResult {
  imported: ImportedIngredient[]
  failed: FailedBarcode[]
}

export const useIngredientsStore = defineStore('ingredients', () => {
  const currentImportJobId = ref<string | null>(null)
  const importStatus = ref<string | null>(null)
  const importProgress = ref<number>(0)
  const importResult = ref<ImportResult | null>(null)
  const isImporting = ref(false)
  const error = ref<string | null>(null)
  const rateLimitInfo = ref<RateLimitInfo | null>(null)

  let pollingInterval: ReturnType<typeof setInterval> | null = null

  function stopPolling() {
    if (pollingInterval !== null) {
      clearInterval(pollingInterval)
      pollingInterval = null
    }
  }

  async function startImport(file: File) {
    error.value = null
    rateLimitInfo.value = null
    importResult.value = null
    importStatus.value = null
    importProgress.value = 0
    isImporting.value = true

    try {
      const result = await importIngredientsFromCsv(file)
      currentImportJobId.value = result.import_job_id
      startPolling(result.import_job_id)
    } catch (e: unknown) {
      isImporting.value = false

      if (isRateLimitError(e)) {
        rateLimitInfo.value = {
          message: e.message,
          retry_after_minutes: e.retry_after_minutes,
        }
      } else {
        error.value = (e as Error)?.message ?? 'Failed to start import.'
      }
    }
  }

  function startPolling(importJobId: string) {
    stopPolling()

    pollingInterval = setInterval(async () => {
      try {
        const response = await getImportJobStatus(importJobId)


        if (response.status === 'rate_limited') {
          stopPolling()
          isImporting.value = false
          rateLimitInfo.value = {
            message: response.message,
            retry_after_minutes: response.retry_after_minutes,
          }
          return
        }

        importStatus.value = response.status
        importProgress.value = response.progress

        if (response.status === 'completed') {
          importProgress.value = 100
          stopPolling()
          isImporting.value = false
          importResult.value = response.result
        } else if (response.status === 'failed') {
          stopPolling()
          isImporting.value = false
          error.value = response.error
        }
      } catch (e: unknown) {
        stopPolling()
        isImporting.value = false

        if (isRateLimitError(e)) {
          rateLimitInfo.value = {
            message: e.message,
            retry_after_minutes: e.retry_after_minutes,
          }
        } else {
          error.value = (e as Error)?.message ?? 'An error occurred while checking import status.'
        }
      }
    }, 2000)
  }

  function clearRateLimit() {
  rateLimitInfo.value = null
  importStatus.value = null
}
  function reset() {
    stopPolling()
    currentImportJobId.value = null
    importStatus.value = null
    importProgress.value = 0
    importResult.value = null
    isImporting.value = false
    error.value = null
    rateLimitInfo.value = null
  }

  return {
    currentImportJobId,
    importStatus,
    importProgress,
    importResult,
    isImporting,
    error,
    rateLimitInfo,
    startImport,
    clearRateLimit,
    reset,
  }
})