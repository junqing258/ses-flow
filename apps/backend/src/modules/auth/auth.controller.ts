import {
  Body,
  Controller,
  Get,
  HttpCode,
  HttpStatus,
  Post,
  UseGuards,
} from '@nestjs/common';
import { AuthGuard } from './auth.guard';
import { AuthService } from './auth.service';
import { CurrentSession } from './current-session.decorator';
import { CurrentUser } from './current-user.decorator';
import type { PublicSession, PublicUser } from './auth.types';

@Controller('auth')
export class AuthController {
  constructor(private readonly authService: AuthService) { }

  @Post('register')
  async register(@Body() body: unknown) {
    return this.authService.register(body);
  }

  @HttpCode(HttpStatus.OK)
  @Post('login')
  async login(@Body() body: unknown) {
    return this.authService.login(body);
  }

  @UseGuards(AuthGuard)
  @Get('me')
  async me(
    @CurrentUser() user: PublicUser,
    @CurrentSession() session: PublicSession,
  ) {
    return { user, session };
  }

  @UseGuards(AuthGuard)
  @HttpCode(HttpStatus.NO_CONTENT)
  @Post('logout')
  async logout(@CurrentSession() session: PublicSession) {
    await this.authService.logoutBySessionId(session.id);
  }
}
