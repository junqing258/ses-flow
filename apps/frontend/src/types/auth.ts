export interface AuthUser {
  id: string;
  email: string;
  displayName: string | null;
  role: "SUPER_ADMIN" | "ADMIN" | "MANAGER" | "OPERATOR" | "VIEWER";
  isActive: boolean;
  lastLoginAt: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface AuthSession {
  id: string;
  expiresAt: string;
  createdAt: string;
  lastUsedAt: string | null;
}

export interface AuthPayload {
  accessToken: string;
  expiresAt: string;
  user: AuthUser;
  session: AuthSession;
}

export interface LoginPayload {
  email: string;
  password: string;
}

export interface RegisterPayload extends LoginPayload {
  displayName?: string;
}

export type AuthDialogMode = "login" | "register";
