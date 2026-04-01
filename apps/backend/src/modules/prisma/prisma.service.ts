import { Injectable, Logger, OnModuleDestroy, OnModuleInit } from '@nestjs/common';
import { ConfigService } from '@nestjs/config';
import { PrismaClient } from '@prisma/client';

@Injectable()
export class PrismaService extends PrismaClient implements OnModuleInit, OnModuleDestroy {
  private readonly logger = new Logger(PrismaService.name);

  constructor(configService: ConfigService) {
    const databaseUrl = configService.get<string>('DATABASE_URL') ?? process.env.DATABASE_URL;
    const nodeEnv = configService.get<string>('NODE_ENV') ?? process.env.NODE_ENV;

    if (!databaseUrl) {
      throw new Error('DATABASE_URL is required to initialize PrismaService');
    }

    super({
      datasources: {
        db: {
          url: databaseUrl,
        },
      },
      log: nodeEnv === 'development' ? ['query', 'info', 'warn', 'error'] : ['warn', 'error'],
      errorFormat: 'minimal',
    });
  }

  async onModuleInit() {
    await this.$connect();
    this.logger.log('Prisma connected to PostgreSQL');
  }

  async onModuleDestroy() {
    await this.$disconnect();
  }
}
