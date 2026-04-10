import { defineStore } from 'pinia'
import { ref } from 'vue'
import { searchRecipe, getJobStatus } from '@/api/recipes'
import type { ActiveContext } from '@/api/recipes'
import type { RateLimitInfo } from '@/types/rateLimit'
import { isRateLimitError } from '@/types/rateLimit'

export const useRecipesStore = defineStore('recipes', () => {
  const currentJobId = ref<string | null>(null)
  const jobStatus = ref<string | null>(null)
  const jobProgress = ref<number>(0)
  const currentRecipe = ref<string | null>(null)
  const currentRecipeCost = ref<number | null>(null)
  const error = ref<string | null>(null)
  const rateLimitInfo = ref<RateLimitInfo | null>(null)

  let pollingInterval: ReturnType<typeof setInterval> | null = null

  function stopPolling() {
    if (pollingInterval !== null) {
      clearInterval(pollingInterval)
      pollingInterval = null
    }
  }

  function startPolling(jobId: string) {
    stopPolling()

    pollingInterval = setInterval(async () => {
      try {
        const response = await getJobStatus(jobId)


        if (response.status === 'rate_limited') {
          stopPolling()
          jobStatus.value = 'rate_limited'
          rateLimitInfo.value = {
            message: response.message,
            retry_after_minutes: response.retry_after_minutes,
          }
          return
        }

        jobStatus.value = response.status
        jobProgress.value = response.progress

        if (response.status === 'completed') {
          jobProgress.value = 100
          stopPolling()
          currentRecipe.value = response.result.markdown_content
          currentRecipeCost.value = response.result.cost_usd
        } else if (response.status === 'failed') {
          stopPolling()
          error.value = response.error
        }
      } catch (e: unknown) {
        stopPolling()

        if (isRateLimitError(e)) {
          jobStatus.value = 'rate_limited'
          rateLimitInfo.value = {
            message: e.message,
            retry_after_minutes: e.retry_after_minutes,
          }
        } else {
          error.value = 'Failed to fetch job status.'
        }
      }
    }, 2000)
  }

  async function startSearch(ctx: ActiveContext): Promise<void> {
    currentJobId.value = null
    jobStatus.value = 'pending'
    jobProgress.value = 0
    currentRecipe.value = null
    currentRecipeCost.value = null
    error.value = null
    rateLimitInfo.value = null
    stopPolling()

    try {
      const result = await searchRecipe(ctx)
      currentJobId.value = result.job_id
      startPolling(result.job_id)
    } catch (e: unknown) {
      if (isRateLimitError(e)) {
        jobStatus.value = 'rate_limited'
        rateLimitInfo.value = {
          message: e.message,
          retry_after_minutes: e.retry_after_minutes,
        }
      } else {
        jobStatus.value = 'failed'
        error.value = 'Failed to start the search.'
      }
    }
  }

 function clearRateLimit() {
  rateLimitInfo.value = null
  jobStatus.value = null
}

  return {
    currentJobId,
    jobStatus,
    jobProgress,
    currentRecipe,
    currentRecipeCost,
    error,
    rateLimitInfo,
    startSearch,
    startPolling,
    clearRateLimit,
  }
})