import { Module } from '@nestjs/common';
import { ConfigModule } from '@nestjs/config';
import { validateEnv, resolveEnvFilePaths } from '../config/env';
import { PrismaModule } from './prisma/prisma.module';
// import { AuthModule } from './auth/auth.module';

@Module({
  imports: [
    ConfigModule.forRoot({
      isGlobal: true,
      cache: true,
      envFilePath: resolveEnvFilePaths(),
      validate: validateEnv,
    }),
    PrismaModule,
    // AuthModule,
  ],
  controllers: [],
  providers: [],
})
export class AppModule { }
