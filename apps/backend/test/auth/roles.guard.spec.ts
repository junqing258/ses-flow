import { ForbiddenException, UnauthorizedException } from '@nestjs/common';
import { Reflector } from '@nestjs/core';
import type { UserRole } from '@prisma/client';
import { RolesGuard, hasRequiredRole } from '../../src/modules/auth/roles.guard';
import type { AuthenticatedRequest, PublicUser } from '../../src/modules/auth/auth.types';

const createUser = (role: UserRole): PublicUser => ({
  id: 'user-1',
  email: 'user@example.com',
  displayName: 'User',
  role,
  isActive: true,
  lastLoginAt: new Date(),
  createdAt: new Date(),
  updatedAt: new Date(),
});

const createExecutionContext = (user?: PublicUser) =>
  ({
    getHandler: () => 'handler',
    getClass: () => 'class',
    switchToHttp: () => ({
      getRequest: () =>
        ({
          headers: {},
          user,
        }) satisfies AuthenticatedRequest,
    }),
  }) as any;

describe('roles.guard', () => {
  it('supports hierarchical role checks', () => {
    expect(hasRequiredRole('SUPER_ADMIN', ['ADMIN'])).toBe(true);
    expect(hasRequiredRole('ADMIN', ['MANAGER'])).toBe(true);
    expect(hasRequiredRole('MANAGER', ['ADMIN'])).toBe(false);
    expect(hasRequiredRole('VIEWER', ['VIEWER'])).toBe(true);
  });

  it('allows access when no role metadata is present', () => {
    const reflector = {
      getAllAndOverride: jest.fn().mockReturnValue(undefined),
    } as unknown as Reflector;
    const guard = new RolesGuard(reflector);

    expect(guard.canActivate(createExecutionContext())).toBe(true);
  });

  it('rejects requests without an authenticated user when roles are required', () => {
    const reflector = {
      getAllAndOverride: jest.fn().mockReturnValue(['ADMIN']),
    } as unknown as Reflector;
    const guard = new RolesGuard(reflector);

    expect(() => guard.canActivate(createExecutionContext())).toThrow(UnauthorizedException);
  });

  it('rejects authenticated users with insufficient privileges', () => {
    const reflector = {
      getAllAndOverride: jest.fn().mockReturnValue(['ADMIN']),
    } as unknown as Reflector;
    const guard = new RolesGuard(reflector);

    expect(() => guard.canActivate(createExecutionContext(createUser('OPERATOR')))).toThrow(
      ForbiddenException,
    );
  });

  it('allows authenticated users with sufficient privileges', () => {
    const reflector = {
      getAllAndOverride: jest.fn().mockReturnValue(['MANAGER']),
    } as unknown as Reflector;
    const guard = new RolesGuard(reflector);

    expect(guard.canActivate(createExecutionContext(createUser('ADMIN')))).toBe(true);
  });
});
