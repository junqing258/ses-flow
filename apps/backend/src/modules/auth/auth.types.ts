import type { AuthSession, User, UserRole } from '@prisma/client';

export interface PublicUser {
  id: User['id'];
  email: User['email'];
  displayName: User['displayName'];
  role: UserRole;
  isActive: User['isActive'];
  lastLoginAt: User['lastLoginAt'];
  createdAt: User['createdAt'];
  updatedAt: User['updatedAt'];
}

export interface PublicSession {
  id: AuthSession['id'];
  expiresAt: AuthSession['expiresAt'];
  createdAt: AuthSession['createdAt'];
  lastUsedAt: AuthSession['lastUsedAt'];
}

export interface AuthResponse {
  accessToken: string;
  expiresAt: Date;
  user: PublicUser;
  session: PublicSession;
}

export interface AuthContext {
  token: string;
  user: PublicUser;
  session: PublicSession;
}

export interface AuthenticatedRequest {
  headers: {
    authorization?: string;
  };
  auth?: AuthContext;
  user?: PublicUser;
  session?: PublicSession;
}
