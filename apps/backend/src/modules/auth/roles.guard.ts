import {
  CanActivate,
  ExecutionContext,
  ForbiddenException,
  Injectable,
  UnauthorizedException,
} from '@nestjs/common';
import { Reflector } from '@nestjs/core';
import type { UserRole } from '@prisma/client';
import { ROLES_KEY } from './roles.decorator';
import type { AuthenticatedRequest, PublicUser } from './auth.types';

const ROLE_PRIORITY: Record<UserRole, number> = {
  VIEWER: 10,
  OPERATOR: 20,
  MANAGER: 30,
  ADMIN: 40,
  SUPER_ADMIN: 50,
};

export const hasRequiredRole = (userRole: UserRole, allowedRoles: UserRole[]) => {
  const userPriority = ROLE_PRIORITY[userRole];

  return allowedRoles.some((role) => userPriority >= ROLE_PRIORITY[role]);
};

@Injectable()
export class RolesGuard implements CanActivate {
  constructor(private readonly reflector: Reflector) { }

  canActivate(context: ExecutionContext): boolean {
    const requiredRoles =
      this.reflector.getAllAndOverride<UserRole[]>(ROLES_KEY, [
        context.getHandler(),
        context.getClass(),
      ]) ?? [];

    if (requiredRoles.length === 0) {
      return true;
    }

    const request = context.switchToHttp().getRequest<AuthenticatedRequest>();
    const user = request.user;

    if (!user) {
      throw new UnauthorizedException('Authenticated user is required');
    }

    if (!hasRequiredRole(user.role, requiredRoles)) {
      throw new ForbiddenException('Insufficient role permissions');
    }

    return true;
  }
}
