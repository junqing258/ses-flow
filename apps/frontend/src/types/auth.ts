export interface AuthUser {
  id: string;
  username?: string;
  email: string;
  displayName: string | null;
  role:
    | "SUPER_ADMIN"
    | "ADMIN"
    | "MANAGER"
    | "OPERATOR"
    | "WORKFLOW_OPERATOR"
    | "WORKSTATION_OPERATOR"
    | "PACKER"
    | "VIEWER";
  roles?: string[];
  permissions?: string[];
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
  login: string;
  password: string;
}

export interface RegisterPayload {
  login: string;
  password: string;
  email?: string;
  displayName?: string;
}

export type AuthDialogMode = "login" | "register";
