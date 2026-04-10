import apiClient from './client';
import type { RateLimitResponse } from '@/types/rateLimit'



export interface GlobalPreferencesDto {
  preference?: string | null;
  country_of_residence_id?: number | null;
  language_id?: number | null;
}

export interface GlobalPreferencesResponse {
  preference?: string | null;
  country?: CountryRow | null;
  language?: LanguageRow | null; 
}
export interface CountryRow {
  id: number;
  code: string;
  name: string;
}
export interface LanguageRow {
  id: number;
  code: string;
  name: string;
}

export interface UserCookingProfile {
  global_preferences: GlobalPreferencesResponse | null;
  ingredients: IngredientRow[];
  appliances: ApplianceRow[];
  cookware: CookwareRow[];
}


export interface IngredientRow {
  id: number;
  name: string;
  fill_percentage: number;
  photo_link: string | null;
  user_id: number;
}

export interface CreateIngredientDto {
  name: string;
  fill_percentage: number;
  photo_link?: string | null;
}

export interface UpdateFillPercentageDto {
  fill_percentage: number;
}

export interface ApplianceRow {
  id: number;
  name: string;
  description: string | null;
  user_id: number;
}

export interface CreateApplianceDto {
  name: string;
  description?: string | null;
}

export interface UpdateApplianceDto {
  name: string;
  description?: string | null;
}

export interface CookwareRow {
  id: number;
  name: string;
  description: string | null;
  user_id: number;
}

export interface CreateCookwareDto {
  name: string;
  description?: string | null;
}

export interface UpdateCookwareDto {
  name: string;
  description?: string | null;
}

export interface ImportedIngredient {
  barcode: string;
  name: string;
  photo_link: string | null;
}

export interface FailedBarcode {
  barcode: string;
  reason: string;
}

export interface ImportJobResult {
  imported: ImportedIngredient[];
  failed: FailedBarcode[];
}
export interface ImportJobResponse {
  import_job_id: string;
  
  status: string;
  
  progress: number;
  result: ImportJobResult | null;
  error?: string | null;
}
export interface ImportJobResponse {
  import_job_id: string;
  status: string;
  result: ImportJobResult | null;
}




export async function getUserCookingProfile(): Promise<UserCookingProfile> {
  const response = await apiClient.get<UserCookingProfile>('/users/me/cooking-profile');
  return response.data;
}

export async function getCountries(): Promise<CountryRow[]> {
  const response = await apiClient.get<CountryRow[]>('/countries');
  return response.data;
}

export async function getLanguages(): Promise<LanguageRow[]> {
  const response = await apiClient.get<LanguageRow[]>('/languages');
  return response.data;
}
export async function updateGlobalPreferences(
  globalPreferences: GlobalPreferencesDto,
): Promise<void> {
  await apiClient.put('/users/me/cooking-profile/global-preferences', globalPreferences);
}



export async function getIngredients(): Promise<IngredientRow[]> {
  const response = await apiClient.get<IngredientRow[]>(
    '/users/me/cooking-profile/ingredients',
  );
  return response.data;
}

export async function addIngredient(ingredient: CreateIngredientDto): Promise<IngredientRow> {
  const response = await apiClient.post<IngredientRow>(
    '/users/me/cooking-profile/ingredients',
    ingredient,
  );
  return response.data;
}

export async function deleteIngredient(ingredientId: number): Promise<void> {
  await apiClient.delete(`/users/me/cooking-profile/ingredients/${ingredientId}`);
}

export async function updateIngredientFillPercentage(
  ingredientId: number,
  fillPercentage: number,
): Promise<IngredientRow> {
  const response = await apiClient.patch<IngredientRow>(
    `/users/me/cooking-profile/ingredients/${ingredientId}/fill-percentage`,
    { fill_percentage: fillPercentage } satisfies UpdateFillPercentageDto,
  );
  return response.data;
}

export async function importIngredientsFromCsv(
  file: File,
): Promise<{ import_job_id: string }> {
  const formData = new FormData()
  formData.append('file', file)
  const response = await apiClient.post<{ import_job_id: string }>(
    '/users/me/cooking-profile/ingredients/import',
    formData,
    { headers: { 'Content-Type': 'multipart/form-data' } },
  )
  return response.data
}

export type ImportJobStatusResponse =
  | { status: 'pending' | 'processing'; progress: number }
  | { status: 'completed'; progress: number; result: ImportJobResult }
  | { status: 'failed'; progress: number; error: string }
  | { status: 'rate_limited'; message: string; retry_after_minutes: number }


export async function getImportJobStatus(
  importJobId: string,
): Promise<ImportJobStatusResponse> {
  const response = await apiClient.get<ImportJobStatusResponse>(
    `/users/me/cooking-profile/ingredients/import/${importJobId}`,
  )
  return response.data
}



export async function getAppliances(): Promise<ApplianceRow[]> {
  const response = await apiClient.get<ApplianceRow[]>(
    '/users/me/cooking-profile/appliances',
  );
  return response.data;
}

export async function addAppliance(appliance: CreateApplianceDto): Promise<ApplianceRow> {
  const response = await apiClient.post<ApplianceRow>(
    '/users/me/cooking-profile/appliances',
    appliance,
  );
  return response.data;
}

export async function updateAppliance(
  applianceId: number,
  appliance: UpdateApplianceDto,
): Promise<ApplianceRow> {
  const response = await apiClient.put<ApplianceRow>(
    `/users/me/cooking-profile/appliances/${applianceId}`,
    appliance,
  );
  return response.data;
}

export async function deleteAppliance(applianceId: number): Promise<void> {
  await apiClient.delete(`/users/me/cooking-profile/appliances/${applianceId}`);
}



export async function getCookware(): Promise<CookwareRow[]> {
  const response = await apiClient.get<CookwareRow[]>(
    '/users/me/cooking-profile/cookware',
  );
  return response.data;
}

export async function addCookware(cookware: CreateCookwareDto): Promise<CookwareRow> {
  const response = await apiClient.post<CookwareRow>(
    '/users/me/cooking-profile/cookware',
    cookware,
  );
  return response.data;
}

export async function updateCookware(
  cookwareId: number,
  cookware: UpdateCookwareDto,
): Promise<CookwareRow> {
  const response = await apiClient.put<CookwareRow>(
    `/users/me/cooking-profile/cookware/${cookwareId}`,
    cookware,
  );
  return response.data;
}

export async function deleteCookware(cookwareId: number): Promise<void> {
  await apiClient.delete(`/users/me/cooking-profile/cookware/${cookwareId}`);
}