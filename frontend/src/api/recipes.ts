import apiClient from './client'

export interface ActiveContext {
  query: string
  kitchen_appliances_id: number
}

export interface SearchRecipeResponse {
  job_id: string
}

export type JobStatus = 'pending' | 'processing' | 'completed' | 'failed' | 'rate_limited'

export interface RecipeResult {
  markdown_content: string
  cost_usd: number
}


export type JobStatusResponse =
  | { status: 'pending' | 'processing'; progress: number }
  | { status: 'completed'; progress: number; result: RecipeResult }
  | { status: 'failed'; progress: number; error: string }
  | { status: 'rate_limited'; message: string; retry_after_minutes: number }


export async function searchRecipe(
  activeContext: ActiveContext,
): Promise<SearchRecipeResponse> {
  const response = await apiClient.post<SearchRecipeResponse>(
    '/recipes/search',
    activeContext,
  )
  return response.data
}


export async function getJobStatus(jobId: string): Promise<JobStatusResponse> {
  const response = await apiClient.get<JobStatusResponse>(`/recipes/jobs/${jobId}`)
  return response.data
}