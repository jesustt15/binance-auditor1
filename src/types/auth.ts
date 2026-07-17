export interface LoginRequest {
  username: string
  password: string
}

export interface LoginResponse {
  token: string
  refresh_token: string
  user: User
}

export interface User {
  id: string
  username: string
  role: 'admin' | 'cashier'
  station_name: string | null
  is_active: boolean
  created_at: string
}

export interface AuthState {
  token: string | null
  refreshToken: string | null
  user: User | null
  isAuthenticated: boolean
  isLoading: boolean
  error: string | null
}
