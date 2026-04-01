import {
  ConflictException,
  Injectable,
  UnauthorizedException,
} from '@nestjs/common';
import { ConfigService } from '@nestjs/config';
import { Prisma, User } from '@prisma/client';
import { PrismaService } from '../prisma/prisma.service';
import { loginSchema, parseAuthInput, registerSchema } from './auth.schemas';
import type {
  AuthContext,
  AuthResponse,
  PublicSession,
  PublicUser,
} from './auth.types';
import {
  createSessionToken,
  extractBearerToken,
  hashPassword,
  hashSessionToken,
  verifyPassword,
} from './auth.utils';

type PrismaExecutor = Prisma.TransactionClient | PrismaService;

@Injectable()
export class AuthService {
  private readonly sessionTtlHours: number;

  constructor(
    private readonly prisma: PrismaService,
    configService: ConfigService,
  ) {
    this.sessionTtlHours = configService.get<number>('AUTH_SESSION_TTL_HOURS') ?? 24;
  }

  async register(payload: unknown): Promise<AuthResponse> {
    const input = parseAuthInput(registerSchema, payload);
    const email = input.email.trim().toLowerCase();
    const now = new Date();

    const existingUser = await this.prisma.user.findUnique({
      where: { email },
      select: { id: true },
    });

    if (existingUser) {
      throw new ConflictException('Email is already registered');
    }

    const passwordHash = await hashPassword(input.password);

    return this.prisma.$transaction(async (tx) => {
      const user = await tx.user.create({
        data: {
          email,
          passwordHash,
          displayName: input.displayName ?? null,
          lastLoginAt: now,
        },
      });

      const session = await this.createSession(tx, user.id, now);
      return this.buildAuthResponse(user, session.token, session.record);
    });
  }

  async login(payload: unknown): Promise<AuthResponse> {
    const input = parseAuthInput(loginSchema, payload);
    const email = input.email.trim().toLowerCase();

    const user = await this.prisma.user.findUnique({
      where: { email },
    });

    if (!user || !user.isActive) {
      throw new UnauthorizedException('Invalid email or password');
    }

    const passwordMatches = await verifyPassword(input.password, user.passwordHash);
    if (!passwordMatches) {
      throw new UnauthorizedException('Invalid email or password');
    }

    const now = new Date();

    return this.prisma.$transaction(async (tx) => {
      const updatedUser = await tx.user.update({
        where: { id: user.id },
        data: { lastLoginAt: now },
      });

      const session = await this.createSession(tx, user.id, now);
      return this.buildAuthResponse(updatedUser, session.token, session.record);
    });
  }

  async validateRequest(authorization?: string): Promise<AuthContext> {
    const token = extractBearerToken(authorization);
    if (!token) {
      throw new UnauthorizedException('Missing bearer token');
    }

    return this.validateAccessToken(token);
  }

  async validateAccessToken(token: string): Promise<AuthContext> {
    const tokenHash = hashSessionToken(token);
    const now = new Date();

    const existingSession = await this.prisma.authSession.findUnique({
      where: { tokenHash },
      include: { user: true },
    });

    if (
      !existingSession ||
      existingSession.revokedAt ||
      existingSession.expiresAt <= now ||
      !existingSession.user.isActive
    ) {
      throw new UnauthorizedException('Session is invalid or expired');
    }

    const updatedSession = await this.prisma.authSession.update({
      where: { id: existingSession.id },
      data: { lastUsedAt: now },
    });

    return {
      token,
      user: this.toPublicUser(existingSession.user),
      session: this.toPublicSession(updatedSession),
    };
  }

  async logoutBySessionId(sessionId: string): Promise<void> {
    await this.prisma.authSession.updateMany({
      where: {
        id: sessionId,
        revokedAt: null,
      },
      data: {
        revokedAt: new Date(),
      },
    });
  }

  private async createSession(prisma: PrismaExecutor, userId: string, now: Date) {
    const token = createSessionToken();
    const record = await prisma.authSession.create({
      data: {
        userId,
        tokenHash: hashSessionToken(token),
        expiresAt: this.buildExpiry(now),
        lastUsedAt: now,
      },
    });

    return { token, record };
  }

  private buildExpiry(now: Date) {
    return new Date(now.getTime() + this.sessionTtlHours * 60 * 60 * 1000);
  }

  private buildAuthResponse(user: User, accessToken: string, session: PublicSession): AuthResponse {
    return {
      accessToken,
      expiresAt: session.expiresAt,
      user: this.toPublicUser(user),
      session,
    };
  }

  private toPublicUser(user: User): PublicUser {
    return {
      id: user.id,
      email: user.email,
      displayName: user.displayName,
      role: user.role,
      isActive: user.isActive,
      lastLoginAt: user.lastLoginAt,
      createdAt: user.createdAt,
      updatedAt: user.updatedAt,
    };
  }

  private toPublicSession(session: {
    id: string;
    expiresAt: Date;
    createdAt: Date;
    lastUsedAt: Date | null;
  }): PublicSession {
    return {
      id: session.id,
      expiresAt: session.expiresAt,
      createdAt: session.createdAt,
      lastUsedAt: session.lastUsedAt,
    };
  }
}
