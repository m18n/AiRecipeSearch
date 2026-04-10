export interface RateLimitResponse {
  status: 'rate_limited';
  message: string;
  retry_after_minutes: number;
}

export interface RateLimitInfo {
  message: string;
  retry_after_minutes: number;
}


export class RateLimitError extends Error {
  readonly retry_after_minutes: number;

  constructor(message: string, retry_after_minutes: number) {
    super(message)
    this.name = 'RateLimitError'
    this.retry_after_minutes = retry_after_minutes
  }
}

export function isRateLimitError(e: unknown): e is RateLimitError {
  return e instanceof RateLimitError
}